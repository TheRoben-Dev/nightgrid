/**
 * CodeMirror 6 setup for the remote/local file editor. Languages are loaded
 * on demand; colours come from the theme's CSS tokens so every theme works.
 * CodeMirror injects its styles through constructable stylesheets
 * (style-mod), which our strict CSP allows.
 */
import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import {
  bracketMatching,
  foldGutter,
  foldKeymap,
  HighlightStyle,
  indentOnInput,
  StreamLanguage,
  syntaxHighlighting,
  type LanguageSupport,
  type StreamParser,
} from "@codemirror/language";
import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  crosshairCursor,
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
} from "@codemirror/view";
import { tags as t } from "@lezer/highlight";
import type { LangKey } from "./util";

const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.moduleKeyword, t.operatorKeyword], color: "var(--magenta)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--mint)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--warn)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--text-1)", fontStyle: "italic" },
  { tag: [t.propertyName, t.attributeName, t.definition(t.propertyName)], color: "var(--cyan)" },
  { tag: [t.typeName, t.className, t.tagName], color: "var(--accent)" },
  { tag: [t.variableName, t.definition(t.variableName)], color: "var(--text-0)" },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: "var(--cyan)" },
  { tag: [t.heading], color: "var(--magenta)", fontWeight: "bold" },
  { tag: [t.meta, t.processingInstruction, t.punctuation, t.separator], color: "var(--text-1)" },
  { tag: [t.link, t.url], color: "var(--cyan)", textDecoration: "underline" },
  { tag: t.invalid, color: "var(--danger)" },
]);

const theme = EditorView.theme(
  {
    "&": { height: "100%", backgroundColor: "var(--bg-0)", color: "var(--text-0)", fontSize: "13px" },
    ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.5" },
    ".cm-content": { caretColor: "var(--magenta)" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--magenta)", borderLeftWidth: "2px" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
      backgroundColor: "color-mix(in srgb, var(--magenta) 28%, transparent) !important",
    },
    ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--text-0) 4%, transparent)" },
    ".cm-gutters": { backgroundColor: "var(--bg-0)", color: "var(--text-1)", borderRight: "1px solid var(--line)" },
    ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--magenta)" },
    ".cm-foldPlaceholder": { backgroundColor: "transparent", border: "1px solid var(--line)", color: "var(--text-1)" },
    ".cm-matchingBracket": { outline: "1px solid var(--cyan)", backgroundColor: "transparent" },
    ".cm-searchMatch": { backgroundColor: "color-mix(in srgb, var(--warn) 30%, transparent)" },
    ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "color-mix(in srgb, var(--magenta) 40%, transparent)" },
    ".cm-selectionMatch": { backgroundColor: "color-mix(in srgb, var(--cyan) 15%, transparent)" },
    ".cm-panels": { backgroundColor: "var(--bg-1)", color: "var(--text-0)", borderColor: "var(--line)" },
    ".cm-panels.cm-panels-bottom": { borderTop: "1px solid var(--line)" },
    ".cm-panel input, .cm-panel button": { fontFamily: "var(--font-mono)", fontSize: "12px" },
    ".cm-textfield": { backgroundColor: "var(--bg-0)", border: "1px solid var(--line)", color: "var(--text-0)" },
    ".cm-button": { backgroundImage: "none", backgroundColor: "var(--bg-2)", border: "1px solid var(--line)", color: "var(--text-0)" },
    ".cm-tooltip": { backgroundColor: "var(--bg-1)", border: "1px solid var(--line)", color: "var(--text-0)" },
    ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--magenta)", color: "var(--on-accent)" },
  },
  { dark: true },
);

/** Loads the language support for a key (code-split per language). */
export async function loadLanguage(key: LangKey): Promise<Extension> {
  const legacy = (p: StreamParser<unknown>) => StreamLanguage.define(p);
  switch (key) {
    case "yaml":
      return (await import("@codemirror/lang-yaml")).yaml();
    case "json":
      return (await import("@codemirror/lang-json")).json();
    case "javascript":
      return (await import("@codemirror/lang-javascript")).javascript();
    case "typescript":
      return (await import("@codemirror/lang-javascript")).javascript({ typescript: true });
    case "python":
      return (await import("@codemirror/lang-python")).python();
    case "markdown":
      return (await import("@codemirror/lang-markdown")).markdown();
    case "html":
      return (await import("@codemirror/lang-html")).html();
    case "css":
      return (await import("@codemirror/lang-css")).css();
    case "xml":
      return (await import("@codemirror/lang-xml")).xml();
    case "shell":
      return legacy((await import("@codemirror/legacy-modes/mode/shell")).shell as StreamParser<unknown>);
    case "nginx":
      return legacy((await import("@codemirror/legacy-modes/mode/nginx")).nginx as StreamParser<unknown>);
    case "dockerfile":
      return legacy((await import("@codemirror/legacy-modes/mode/dockerfile")).dockerFile as StreamParser<unknown>);
    case "toml":
      return legacy((await import("@codemirror/legacy-modes/mode/toml")).toml as StreamParser<unknown>);
    case "properties":
      return legacy((await import("@codemirror/legacy-modes/mode/properties")).properties as StreamParser<unknown>);
    default:
      return [];
  }
}

export interface EditorHooks {
  onsave: () => void;
  onchange: () => void;
}

export interface EditorHandle {
  view: EditorView;
  setLanguage: (key: LangKey) => Promise<void>;
  /** Current text, with the file's original line endings. */
  text: () => string;
  /** Replaces the whole document (after a reload) without adding undo history. */
  reset: (content: string) => void;
}

/** Line separator the file uses (kept on save). */
export const lineEnding = (content: string): "\r\n" | "\n" => (/\r\n/.test(content) ? "\r\n" : "\n");

export function createEditor(parent: HTMLElement, content: string, lang: LangKey, hooks: EditorHooks): EditorHandle {
  const language = new Compartment();
  const sep = lineEnding(content);
  const extensions = (): Extension[] => [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    foldGutter(),
    drawSelection(),
    EditorState.allowMultipleSelections.of(true),
    EditorState.lineSeparator.of(sep),
    indentOnInput(),
    syntaxHighlighting(highlight),
    bracketMatching(),
    closeBrackets(),
    autocompletion({ activateOnTyping: false }),
    rectangularSelection(),
    crosshairCursor(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    search({ top: false }),
    keymap.of([
      { key: "Mod-s", preventDefault: true, run: () => (hooks.onsave(), true) },
      ...closeBracketsKeymap,
      ...defaultKeymap,
      ...searchKeymap,
      ...historyKeymap,
      ...foldKeymap,
      ...completionKeymap,
      indentWithTab,
    ]),
    theme,
    language.of([]),
    EditorView.updateListener.of((u) => {
      if (u.docChanged) hooks.onchange();
    }),
  ];
  const view = new EditorView({ parent, state: EditorState.create({ doc: content, extensions: extensions() }) });
  const handle: EditorHandle = {
    view,
    async setLanguage(key) {
      const ext = await loadLanguage(key);
      view.dispatch({ effects: language.reconfigure(ext as LanguageSupport | Extension) });
    },
    text: () => view.state.sliceDoc(),
    reset(next) {
      view.setState(EditorState.create({ doc: next, extensions: extensions() }));
      void handle.setLanguage(lang);
    },
  };
  void handle.setLanguage(lang);
  return handle;
}
