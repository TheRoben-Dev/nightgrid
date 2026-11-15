//! The helper's local model runtime: llama.cpp, embedded (Metal on macOS, CPU
//! elsewhere for now). Everything here is blocking; callers run it on a
//! blocking thread. The crate knows nothing about Nightgrid's prompts: it
//! loads a GGUF file, applies the model's own chat template and streams text.

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaChatTemplate, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("the llama.cpp backend couldn't start: {0}")]
    Backend(String),
    #[error("the model couldn't be loaded: {0}")]
    Load(String),
    #[error("the prompt couldn't be prepared: {0}")]
    Prompt(String),
    #[error("the conversation is too long for the model ({tokens} of {limit} tokens); start a new one")]
    TooLong { tokens: usize, limit: usize },
    #[error("generation failed: {0}")]
    Decode(String),
}

pub type Result<T> = std::result::Result<T, LlmError>;

/// One chat turn. `role` is "system", "user" or "assistant".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self { role: role.to_owned(), content: content.into() }
    }
}

#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// Layers to offload to the GPU; `u32::MAX` means all of them. Ignored on
    /// builds without a GPU backend.
    pub gpu_layers: u32,
    /// Context window in tokens.
    pub context: u32,
    /// CPU threads; 0 picks a sensible number.
    pub threads: u32,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self { gpu_layers: u32::MAX, context: 8192, threads: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct GenOptions {
    pub max_tokens: u32,
    pub temperature: f32,
    pub seed: u32,
}

impl Default for GenOptions {
    fn default() -> Self {
        Self { max_tokens: 768, temperature: 0.4, seed: 0x5eed }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// The model ended its turn.
    Done,
    /// `max_tokens` was reached.
    Length,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub text: String,
    pub finish: Finish,
    pub prompt_tokens: usize,
    pub tokens: usize,
}

/// Whether this build offloads to a GPU (Metal on macOS).
pub fn gpu_build() -> bool {
    cfg!(target_os = "macos")
}

fn backend() -> Result<&'static LlamaBackend> {
    static BACKEND: OnceLock<std::result::Result<LlamaBackend, String>> = OnceLock::new();
    BACKEND
        .get_or_init(|| {
            // llama.cpp is chatty on stderr; route it through tracing, where
            // it lands in the log file at debug level.
            llama_cpp_2::send_logs_to_tracing(llama_cpp_2::LogOptions::default());
            LlamaBackend::init().map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| LlmError::Backend(e.clone()))
}

fn default_threads() -> i32 {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get());
    // Leave a core or two for the UI and SSH sessions.
    n.saturating_sub(if n > 4 { 2 } else { 1 }).clamp(1, 16) as i32
}

pub struct Engine {
    model: LlamaModel,
    template: LlamaChatTemplate,
    opts: LoadOptions,
}

impl Engine {
    /// Loads a GGUF model. Takes seconds and as much memory as the file.
    pub fn load(path: &Path, opts: LoadOptions) -> Result<Self> {
        let backend = backend()?;
        let params = LlamaModelParams::default().with_n_gpu_layers(if gpu_build() { opts.gpu_layers } else { 0 });
        let model = LlamaModel::load_from_file(backend, path, &params).map_err(|e| LlmError::Load(e.to_string()))?;
        // The template baked into the file is the right one; ChatML is the
        // fallback llama.cpp understands for most instruct models.
        let template = model
            .chat_template(None)
            .or_else(|_| LlamaChatTemplate::new("chatml").map_err(|e| LlmError::Prompt(e.to_string())))
            .map_err(|e| LlmError::Prompt(e.to_string()))?;
        Ok(Self { model, template, opts })
    }

    /// Context window in tokens.
    pub fn context(&self) -> u32 {
        self.opts.context.min(self.model.n_ctx_train().max(512))
    }

    /// Renders the chat with the model's template, ending with an open
    /// assistant turn.
    pub fn render(&self, messages: &[Message]) -> Result<String> {
        let chat = messages
            .iter()
            .map(|m| LlamaChatMessage::new(m.role.clone(), m.content.clone()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| LlmError::Prompt(e.to_string()))?;
        self.model.apply_chat_template(&self.template, &chat, true).map_err(|e| LlmError::Prompt(e.to_string()))
    }

    /// Tokens `text` takes.
    pub fn count_tokens(&self, text: &str) -> usize {
        self.model.vocab().tokenize(text.as_bytes(), false, true).len()
    }

    /// Generates the assistant's reply, calling `on_text` with each new piece
    /// of valid UTF-8 as it is produced. Stops at the end of the turn, at
    /// `max_tokens`, or soon after `cancel` is set.
    pub fn generate(
        &self,
        messages: &[Message],
        gen: &GenOptions,
        cancel: &AtomicBool,
        mut on_text: impl FnMut(&str),
    ) -> Result<Outcome> {
        let backend = backend()?;
        let prompt = self.render(messages)?;
        let vocab = self.model.vocab();
        let tokens = vocab.tokenize(prompt.as_bytes(), vocab.should_add_bos(), true);
        let n_ctx = self.context();
        let limit = n_ctx as usize;
        if tokens.len() + 16 > limit {
            return Err(LlmError::TooLong { tokens: tokens.len(), limit });
        }
        let threads = if self.opts.threads == 0 { default_threads() } else { self.opts.threads as i32 };
        let n_batch = 512u32;
        let cparams = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(n_ctx))
            .with_n_batch(n_batch)
            .with_n_threads(threads)
            .with_n_threads_batch(threads);
        let mut ctx = self.model.new_context(backend, cparams).map_err(|e| LlmError::Load(e.to_string()))?;

        // Prompt, in batches.
        let mut batch = LlamaBatch::new(n_batch as usize, 1);
        let last = tokens.len() - 1;
        for (start, chunk) in tokens.chunks(n_batch as usize).enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Ok(Outcome {
                    text: String::new(),
                    finish: Finish::Cancelled,
                    prompt_tokens: tokens.len(),
                    tokens: 0,
                });
            }
            batch.clear();
            for (j, t) in chunk.iter().enumerate() {
                let pos = start * n_batch as usize + j;
                batch.add(*t, pos as i32, &[0], pos == last).map_err(|e| LlmError::Decode(e.to_string()))?;
            }
            ctx.decode(&mut batch).map_err(|e| LlmError::Decode(e.to_string()))?;
        }

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::top_k(40),
            LlamaSampler::top_p(0.9, 1),
            LlamaSampler::min_p(0.05, 1),
            LlamaSampler::temp(gen.temperature),
            LlamaSampler::dist(gen.seed),
        ]);

        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut piece = Vec::new();
        let mut text = String::new();
        let mut pos = tokens.len() as i32;
        let mut produced = 0usize;
        let room = (limit - tokens.len()).min(gen.max_tokens as usize);
        let finish = loop {
            if cancel.load(Ordering::Relaxed) {
                break Finish::Cancelled;
            }
            if produced >= room {
                break Finish::Length;
            }
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            if vocab.is_eog(token) {
                break Finish::Done;
            }
            produced += 1;
            piece.clear();
            vocab.token_to_piece_into(token, &mut piece, false, None);
            let mut out = String::with_capacity(decoder.max_utf8_buffer_length(piece.len()).unwrap_or(piece.len() * 3));
            let _ = decoder.decode_to_string(&piece, &mut out, false);
            if !out.is_empty() {
                on_text(&out);
                text.push_str(&out);
            }
            batch.clear();
            batch.add(token, pos, &[0], true).map_err(|e| LlmError::Decode(e.to_string()))?;
            pos += 1;
            ctx.decode(&mut batch).map_err(|e| LlmError::Decode(e.to_string()))?;
        };
        Ok(Outcome { text, finish, prompt_tokens: tokens.len(), tokens: produced })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs only with `NIGHTGRID_TEST_LLM_MODEL=/path/to/model.gguf` (CI
    /// downloads a tiny pinned model for it).
    #[test]
    fn generates_with_a_real_model() {
        let Ok(path) = std::env::var("NIGHTGRID_TEST_LLM_MODEL") else {
            eprintln!("skipped: set NIGHTGRID_TEST_LLM_MODEL");
            return;
        };
        let engine = Engine::load(Path::new(&path), LoadOptions { context: 2048, ..Default::default() }).unwrap();
        let msgs = [
            Message::new("system", "You are terse. Answer in one short sentence."),
            Message::new("user", "What colour is the sky on a clear day?"),
        ];
        let prompt = engine.render(&msgs).unwrap();
        assert!(prompt.contains("clear day"), "{prompt}");
        let cancel = AtomicBool::new(false);
        let mut streamed = String::new();
        let out = engine
            .generate(&msgs, &GenOptions { max_tokens: 48, temperature: 0.0, seed: 1 }, &cancel, |t| {
                streamed.push_str(t)
            })
            .unwrap();
        assert_eq!(streamed, out.text);
        assert!(out.tokens > 0 && out.prompt_tokens > 10, "{out:?}");
        assert!(matches!(out.finish, Finish::Done | Finish::Length));

        // A cancelled run stops at once.
        cancel.store(true, Ordering::Relaxed);
        let out = engine.generate(&msgs, &GenOptions::default(), &cancel, |_| {}).unwrap();
        assert_eq!(out.finish, Finish::Cancelled);

        // An over-long conversation is refused, not truncated.
        let long = Message::new("user", "word ".repeat(4000));
        let e = engine.generate(&[long], &GenOptions::default(), &AtomicBool::new(false), |_| {}).unwrap_err();
        assert!(matches!(e, LlmError::TooLong { .. }), "{e}");
    }
}
