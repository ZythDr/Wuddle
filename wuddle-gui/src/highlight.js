/**
 * Lightweight highlight.js bundle — only the languages we need.
 * Using the core + selective language registration to minimise bundle size.
 */
import hljs from "highlight.js/lib/core";

import lua from "highlight.js/lib/languages/lua";
import xml from "highlight.js/lib/languages/xml";       // also covers HTML
import markdown from "highlight.js/lib/languages/markdown";
import css from "highlight.js/lib/languages/css";
import javascript from "highlight.js/lib/languages/javascript";
import ini from "highlight.js/lib/languages/ini";        // covers .toc-style key=value
import diff from "highlight.js/lib/languages/diff";
import plaintext from "highlight.js/lib/languages/plaintext";

hljs.registerLanguage("lua", lua);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("css", css);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("ini", ini);
hljs.registerLanguage("diff", diff);
hljs.registerLanguage("plaintext", plaintext);

/** Map file extensions to highlight.js language names. */
const EXT_MAP = {
  lua:  "lua",
  xml:  "xml",
  html: "xml",
  htm:  "xml",
  toc:  "ini",
  md:   "markdown",
  markdown: "markdown",
  css:  "css",
  js:   "javascript",
  mjs:  "javascript",
  json: "javascript",
  diff: "diff",
  patch: "diff",
  txt:  "plaintext",
  log:  "plaintext",
};

/**
 * Highlight source code and return an HTML string.
 * Falls back to auto-detect if extension is unknown.
 */
export function highlightCode(code, filename) {
  const ext = (filename.split(".").pop() || "").toLowerCase();
  const lang = EXT_MAP[ext];

  if (lang) {
    try {
      return hljs.highlight(code, { language: lang }).value;
    } catch (_) {
      // fall through to auto
    }
  }

  // Try auto-detection; fall back to escaped plain text
  try {
    const result = hljs.highlightAuto(code);
    if (result.relevance > 2) return result.value;
  } catch (_) {
    // ignore
  }
  return null; // caller should escapeHtml
}
