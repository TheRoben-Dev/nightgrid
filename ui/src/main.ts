// Fonts are bundled from npm (OFL); nothing is loaded from a CDN.
import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-500.css";
// Bold and accented text in terminals: a real face keeps cells aligned (no synthesized bold).
import "@fontsource/jetbrains-mono/latin-700.css";
import "@fontsource/jetbrains-mono/latin-ext-400.css";
import "@fontsource/rajdhani/latin-500.css";
import "@fontsource/rajdhani/latin-600.css";
import "@fontsource/rajdhani/latin-700.css";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/components.css";
import "./styles/backdrop.css";

import { mount } from "svelte";
import App from "./App.svelte";
import { installCspStyleMirror } from "./lib/term/cspStyles";

// Before anything can inject <style> elements (see cspStyles.ts).
installCspStyleMirror();

const app = mount(App, { target: document.getElementById("app")! });

export default app;

// Fetch the terminal fonts up front so the first prompt is measured and drawn
// with them (no fallback-sized cells, no cached tofu).
void document.fonts?.load('13px "NG Symbols"', "\ue0b0").catch(() => {});
void document.fonts?.load('13px "JetBrains Mono"').catch(() => {});
void document.fonts?.load('bold 13px "JetBrains Mono"').catch(() => {});
