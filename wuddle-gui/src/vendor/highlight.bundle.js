function Yn(s) {
  return s && s.__esModule && Object.prototype.hasOwnProperty.call(s, "default") ? s.default : s;
}
var Pe, en;
function qn() {
  if (en) return Pe;
  en = 1;
  function s(e) {
    return e instanceof Map ? e.clear = e.delete = e.set = function() {
      throw new Error("map is read-only");
    } : e instanceof Set && (e.add = e.clear = e.delete = function() {
      throw new Error("set is read-only");
    }), Object.freeze(e), Object.getOwnPropertyNames(e).forEach((n) => {
      const i = e[n], g = typeof i;
      (g === "object" || g === "function") && !Object.isFrozen(i) && s(i);
    }), e;
  }
  class f {
    /**
     * @param {CompiledMode} mode
     */
    constructor(n) {
      n.data === void 0 && (n.data = {}), this.data = n.data, this.isMatchIgnored = !1;
    }
    ignoreMatch() {
      this.isMatchIgnored = !0;
    }
  }
  function E(e) {
    return e.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#x27;");
  }
  function b(e, ...n) {
    const i = /* @__PURE__ */ Object.create(null);
    for (const g in e)
      i[g] = e[g];
    return n.forEach(function(g) {
      for (const y in g)
        i[y] = g[y];
    }), /** @type {T} */
    i;
  }
  const x = "</span>", L = (e) => !!e.scope, A = (e, { prefix: n }) => {
    if (e.startsWith("language:"))
      return e.replace("language:", "language-");
    if (e.includes(".")) {
      const i = e.split(".");
      return [
        `${n}${i.shift()}`,
        ...i.map((g, y) => `${g}${"_".repeat(y + 1)}`)
      ].join(" ");
    }
    return `${n}${e}`;
  };
  class S {
    /**
     * Creates a new HTMLRenderer
     *
     * @param {Tree} parseTree - the parse tree (must support `walk` API)
     * @param {{classPrefix: string}} options
     */
    constructor(n, i) {
      this.buffer = "", this.classPrefix = i.classPrefix, n.walk(this);
    }
    /**
     * Adds texts to the output stream
     *
     * @param {string} text */
    addText(n) {
      this.buffer += E(n);
    }
    /**
     * Adds a node open to the output stream (if needed)
     *
     * @param {Node} node */
    openNode(n) {
      if (!L(n)) return;
      const i = A(
        n.scope,
        { prefix: this.classPrefix }
      );
      this.span(i);
    }
    /**
     * Adds a node close to the output stream (if needed)
     *
     * @param {Node} node */
    closeNode(n) {
      L(n) && (this.buffer += x);
    }
    /**
     * returns the accumulated buffer
    */
    value() {
      return this.buffer;
    }
    // helpers
    /**
     * Builds a span element
     *
     * @param {string} className */
    span(n) {
      this.buffer += `<span class="${n}">`;
    }
  }
  const C = (e = {}) => {
    const n = { children: [] };
    return Object.assign(n, e), n;
  };
  class v {
    constructor() {
      this.rootNode = C(), this.stack = [this.rootNode];
    }
    get top() {
      return this.stack[this.stack.length - 1];
    }
    get root() {
      return this.rootNode;
    }
    /** @param {Node} node */
    add(n) {
      this.top.children.push(n);
    }
    /** @param {string} scope */
    openNode(n) {
      const i = C({ scope: n });
      this.add(i), this.stack.push(i);
    }
    closeNode() {
      if (this.stack.length > 1)
        return this.stack.pop();
    }
    closeAllNodes() {
      for (; this.closeNode(); ) ;
    }
    toJSON() {
      return JSON.stringify(this.rootNode, null, 4);
    }
    /**
     * @typedef { import("./html_renderer").Renderer } Renderer
     * @param {Renderer} builder
     */
    walk(n) {
      return this.constructor._walk(n, this.rootNode);
    }
    /**
     * @param {Renderer} builder
     * @param {Node} node
     */
    static _walk(n, i) {
      return typeof i == "string" ? n.addText(i) : i.children && (n.openNode(i), i.children.forEach((g) => this._walk(n, g)), n.closeNode(i)), n;
    }
    /**
     * @param {Node} node
     */
    static _collapse(n) {
      typeof n != "string" && n.children && (n.children.every((i) => typeof i == "string") ? n.children = [n.children.join("")] : n.children.forEach((i) => {
        v._collapse(i);
      }));
    }
  }
  class U extends v {
    /**
     * @param {*} options
     */
    constructor(n) {
      super(), this.options = n;
    }
    /**
     * @param {string} text
     */
    addText(n) {
      n !== "" && this.add(n);
    }
    /** @param {string} scope */
    startScope(n) {
      this.openNode(n);
    }
    endScope() {
      this.closeNode();
    }
    /**
     * @param {Emitter & {root: DataNode}} emitter
     * @param {string} name
     */
    __addSublanguage(n, i) {
      const g = n.root;
      i && (g.scope = `language:${i}`), this.add(g);
    }
    toHTML() {
      return new S(this, this.options).value();
    }
    finalize() {
      return this.closeAllNodes(), !0;
    }
  }
  function D(e) {
    return e ? typeof e == "string" ? e : e.source : null;
  }
  function B(e) {
    return $("(?=", e, ")");
  }
  function z(e) {
    return $("(?:", e, ")*");
  }
  function oe(e) {
    return $("(?:", e, ")?");
  }
  function $(...e) {
    return e.map((i) => D(i)).join("");
  }
  function ce(e) {
    const n = e[e.length - 1];
    return typeof n == "object" && n.constructor === Object ? (e.splice(e.length - 1, 1), n) : {};
  }
  function Y(...e) {
    return "(" + (ce(e).capture ? "" : "?:") + e.map((g) => D(g)).join("|") + ")";
  }
  function te(e) {
    return new RegExp(e.toString() + "|").exec("").length - 1;
  }
  function ge(e, n) {
    const i = e && e.exec(n);
    return i && i.index === 0;
  }
  const ue = /\[(?:[^\\\]]|\\.)*\]|\(\??|\\([1-9][0-9]*)|\\./;
  function q(e, { joinWith: n }) {
    let i = 0;
    return e.map((g) => {
      i += 1;
      const y = i;
      let w = D(g), o = "";
      for (; w.length > 0; ) {
        const a = ue.exec(w);
        if (!a) {
          o += w;
          break;
        }
        o += w.substring(0, a.index), w = w.substring(a.index + a[0].length), a[0][0] === "\\" && a[1] ? o += "\\" + String(Number(a[1]) + y) : (o += a[0], a[0] === "(" && i++);
      }
      return o;
    }).map((g) => `(${g})`).join(n);
  }
  const Q = /\b\B/, fe = "[a-zA-Z]\\w*", ie = "[a-zA-Z_]\\w*", be = "\\b\\d+(\\.\\d+)?", pe = "(-?)(\\b0[xX][a-fA-F0-9]+|(\\b\\d+(\\.\\d*)?|\\.\\d+)([eE][-+]?\\d+)?)", he = "\\b(0b[01]+)", Se = "!|!=|!==|%|%=|&|&&|&=|\\*|\\*=|\\+|\\+=|,|-|-=|/=|/|:|;|<<|<<=|<=|<|===|==|=|>>>=|>>=|>=|>>>|>>|>|\\?|\\[|\\{|\\(|\\^|\\^=|\\||\\|=|\\|\\||~", ke = (e = {}) => {
    const n = /^#![ ]*\//;
    return e.binary && (e.begin = $(
      n,
      /.*\b/,
      e.binary,
      /\b.*/
    )), b({
      scope: "meta",
      begin: n,
      end: /$/,
      relevance: 0,
      /** @type {ModeCallback} */
      "on:begin": (i, g) => {
        i.index !== 0 && g.ignoreMatch();
      }
    }, e);
  }, V = {
    begin: "\\\\[\\s\\S]",
    relevance: 0
  }, Te = {
    scope: "string",
    begin: "'",
    end: "'",
    illegal: "\\n",
    contains: [V]
  }, Ee = {
    scope: "string",
    begin: '"',
    end: '"',
    illegal: "\\n",
    contains: [V]
  }, Ae = {
    begin: /\b(a|an|the|are|I'm|isn't|don't|doesn't|won't|but|just|should|pretty|simply|enough|gonna|going|wtf|so|such|will|you|your|they|like|more)\b/
  }, M = function(e, n, i = {}) {
    const g = b(
      {
        scope: "comment",
        begin: e,
        end: n,
        contains: []
      },
      i
    );
    g.contains.push({
      scope: "doctag",
      // hack to avoid the space from being included. the space is necessary to
      // match here to prevent the plain text rule below from gobbling up doctags
      begin: "[ ]*(?=(TODO|FIXME|NOTE|BUG|OPTIMIZE|HACK|XXX):)",
      end: /(TODO|FIXME|NOTE|BUG|OPTIMIZE|HACK|XXX):/,
      excludeBegin: !0,
      relevance: 0
    });
    const y = Y(
      // list of common 1 and 2 letter words in English
      "I",
      "a",
      "is",
      "so",
      "us",
      "to",
      "at",
      "if",
      "in",
      "it",
      "on",
      // note: this is not an exhaustive list of contractions, just popular ones
      /[A-Za-z]+['](d|ve|re|ll|t|s|n)/,
      // contractions - can't we'd they're let's, etc
      /[A-Za-z]+[-][a-z]+/,
      // `no-way`, etc.
      /[A-Za-z][a-z]{2,}/
      // allow capitalized words at beginning of sentences
    );
    return g.contains.push(
      {
        // TODO: how to include ", (, ) without breaking grammars that use these for
        // comment delimiters?
        // begin: /[ ]+([()"]?([A-Za-z'-]{3,}|is|a|I|so|us|[tT][oO]|at|if|in|it|on)[.]?[()":]?([.][ ]|[ ]|\))){3}/
        // ---
        // this tries to find sequences of 3 english words in a row (without any
        // "programming" type syntax) this gives us a strong signal that we've
        // TRULY found a comment - vs perhaps scanning with the wrong language.
        // It's possible to find something that LOOKS like the start of the
        // comment - but then if there is no readable text - good chance it is a
        // false match and not a comment.
        //
        // for a visual example please see:
        // https://github.com/highlightjs/highlight.js/issues/2827
        begin: $(
          /[ ]+/,
          // necessary to prevent us gobbling up doctags like /* @author Bob Mcgill */
          "(",
          y,
          /[.]?[:]?([.][ ]|[ ])/,
          "){3}"
        )
        // look for 3 words in a row
      }
    ), g;
  }, j = M("//", "$"), J = M("/\\*", "\\*/"), re = M("#", "$"), le = {
    scope: "number",
    begin: be,
    relevance: 0
  }, me = {
    scope: "number",
    begin: pe,
    relevance: 0
  }, an = {
    scope: "number",
    begin: he,
    relevance: 0
  }, on = {
    scope: "regexp",
    begin: /\/(?=[^/\n]*\/)/,
    end: /\/[gimuy]*/,
    contains: [
      V,
      {
        begin: /\[/,
        end: /\]/,
        relevance: 0,
        contains: [V]
      }
    ]
  }, cn = {
    scope: "title",
    begin: fe,
    relevance: 0
  }, ln = {
    scope: "title",
    begin: ie,
    relevance: 0
  }, dn = {
    // excludes method names from keyword processing
    begin: "\\.\\s*" + ie,
    relevance: 0
  };
  var _e = /* @__PURE__ */ Object.freeze({
    __proto__: null,
    APOS_STRING_MODE: Te,
    BACKSLASH_ESCAPE: V,
    BINARY_NUMBER_MODE: an,
    BINARY_NUMBER_RE: he,
    COMMENT: M,
    C_BLOCK_COMMENT_MODE: J,
    C_LINE_COMMENT_MODE: j,
    C_NUMBER_MODE: me,
    C_NUMBER_RE: pe,
    END_SAME_AS_BEGIN: function(e) {
      return Object.assign(
        e,
        {
          /** @type {ModeCallback} */
          "on:begin": (n, i) => {
            i.data._beginMatch = n[1];
          },
          /** @type {ModeCallback} */
          "on:end": (n, i) => {
            i.data._beginMatch !== n[1] && i.ignoreMatch();
          }
        }
      );
    },
    HASH_COMMENT_MODE: re,
    IDENT_RE: fe,
    MATCH_NOTHING_RE: Q,
    METHOD_GUARD: dn,
    NUMBER_MODE: le,
    NUMBER_RE: be,
    PHRASAL_WORDS_MODE: Ae,
    QUOTE_STRING_MODE: Ee,
    REGEXP_MODE: on,
    RE_STARTERS_RE: Se,
    SHEBANG: ke,
    TITLE_MODE: cn,
    UNDERSCORE_IDENT_RE: ie,
    UNDERSCORE_TITLE_MODE: ln
  });
  function gn(e, n) {
    e.input[e.index - 1] === "." && n.ignoreMatch();
  }
  function un(e, n) {
    e.className !== void 0 && (e.scope = e.className, delete e.className);
  }
  function fn(e, n) {
    n && e.beginKeywords && (e.begin = "\\b(" + e.beginKeywords.split(" ").join("|") + ")(?!\\.)(?=\\b|\\s)", e.__beforeBegin = gn, e.keywords = e.keywords || e.beginKeywords, delete e.beginKeywords, e.relevance === void 0 && (e.relevance = 0));
  }
  function bn(e, n) {
    Array.isArray(e.illegal) && (e.illegal = Y(...e.illegal));
  }
  function pn(e, n) {
    if (e.match) {
      if (e.begin || e.end) throw new Error("begin & end are not supported with match");
      e.begin = e.match, delete e.match;
    }
  }
  function hn(e, n) {
    e.relevance === void 0 && (e.relevance = 1);
  }
  const En = (e, n) => {
    if (!e.beforeMatch) return;
    if (e.starts) throw new Error("beforeMatch cannot be used with starts");
    const i = Object.assign({}, e);
    Object.keys(e).forEach((g) => {
      delete e[g];
    }), e.keywords = i.keywords, e.begin = $(i.beforeMatch, B(i.begin)), e.starts = {
      relevance: 0,
      contains: [
        Object.assign(i, { endsParent: !0 })
      ]
    }, e.relevance = 0, delete i.beforeMatch;
  }, mn = [
    "of",
    "and",
    "for",
    "in",
    "not",
    "or",
    "if",
    "then",
    "parent",
    // common variable name
    "list",
    // common variable name
    "value"
    // common variable name
  ], _n = "keyword";
  function Ue(e, n, i = _n) {
    const g = /* @__PURE__ */ Object.create(null);
    return typeof e == "string" ? y(i, e.split(" ")) : Array.isArray(e) ? y(i, e) : Object.keys(e).forEach(function(w) {
      Object.assign(
        g,
        Ue(e[w], n, w)
      );
    }), g;
    function y(w, o) {
      n && (o = o.map((a) => a.toLowerCase())), o.forEach(function(a) {
        const d = a.split("|");
        g[d[0]] = [w, Nn(d[0], d[1])];
      });
    }
  }
  function Nn(e, n) {
    return n ? Number(n) : xn(e) ? 0 : 1;
  }
  function xn(e) {
    return mn.includes(e.toLowerCase());
  }
  const ze = {}, ee = (e) => {
    console.error(e);
  }, $e = (e, ...n) => {
    console.log(`WARN: ${e}`, ...n);
  }, se = (e, n) => {
    ze[`${e}/${n}`] || (console.log(`Deprecated as of ${e}. ${n}`), ze[`${e}/${n}`] = !0);
  }, Ne = new Error();
  function Ge(e, n, { key: i }) {
    let g = 0;
    const y = e[i], w = {}, o = {};
    for (let a = 1; a <= n.length; a++)
      o[a + g] = y[a], w[a + g] = !0, g += te(n[a - 1]);
    e[i] = o, e[i]._emit = w, e[i]._multi = !0;
  }
  function yn(e) {
    if (Array.isArray(e.begin)) {
      if (e.skip || e.excludeBegin || e.returnBegin)
        throw ee("skip, excludeBegin, returnBegin not compatible with beginScope: {}"), Ne;
      if (typeof e.beginScope != "object" || e.beginScope === null)
        throw ee("beginScope must be object"), Ne;
      Ge(e, e.begin, { key: "beginScope" }), e.begin = q(e.begin, { joinWith: "" });
    }
  }
  function wn(e) {
    if (Array.isArray(e.end)) {
      if (e.skip || e.excludeEnd || e.returnEnd)
        throw ee("skip, excludeEnd, returnEnd not compatible with endScope: {}"), Ne;
      if (typeof e.endScope != "object" || e.endScope === null)
        throw ee("endScope must be object"), Ne;
      Ge(e, e.end, { key: "endScope" }), e.end = q(e.end, { joinWith: "" });
    }
  }
  function vn(e) {
    e.scope && typeof e.scope == "object" && e.scope !== null && (e.beginScope = e.scope, delete e.scope);
  }
  function On(e) {
    vn(e), typeof e.beginScope == "string" && (e.beginScope = { _wrap: e.beginScope }), typeof e.endScope == "string" && (e.endScope = { _wrap: e.endScope }), yn(e), wn(e);
  }
  function Sn(e) {
    function n(o, a) {
      return new RegExp(
        D(o),
        "m" + (e.case_insensitive ? "i" : "") + (e.unicodeRegex ? "u" : "") + (a ? "g" : "")
      );
    }
    class i {
      constructor() {
        this.matchIndexes = {}, this.regexes = [], this.matchAt = 1, this.position = 0;
      }
      // @ts-ignore
      addRule(a, d) {
        d.position = this.position++, this.matchIndexes[this.matchAt] = d, this.regexes.push([d, a]), this.matchAt += te(a) + 1;
      }
      compile() {
        this.regexes.length === 0 && (this.exec = () => null);
        const a = this.regexes.map((d) => d[1]);
        this.matcherRe = n(q(a, { joinWith: "|" }), !0), this.lastIndex = 0;
      }
      /** @param {string} s */
      exec(a) {
        this.matcherRe.lastIndex = this.lastIndex;
        const d = this.matcherRe.exec(a);
        if (!d)
          return null;
        const T = d.findIndex((de, Re) => Re > 0 && de !== void 0), O = this.matchIndexes[T];
        return d.splice(0, T), Object.assign(d, O);
      }
    }
    class g {
      constructor() {
        this.rules = [], this.multiRegexes = [], this.count = 0, this.lastIndex = 0, this.regexIndex = 0;
      }
      // @ts-ignore
      getMatcher(a) {
        if (this.multiRegexes[a]) return this.multiRegexes[a];
        const d = new i();
        return this.rules.slice(a).forEach(([T, O]) => d.addRule(T, O)), d.compile(), this.multiRegexes[a] = d, d;
      }
      resumingScanAtSamePosition() {
        return this.regexIndex !== 0;
      }
      considerAll() {
        this.regexIndex = 0;
      }
      // @ts-ignore
      addRule(a, d) {
        this.rules.push([a, d]), d.type === "begin" && this.count++;
      }
      /** @param {string} s */
      exec(a) {
        const d = this.getMatcher(this.regexIndex);
        d.lastIndex = this.lastIndex;
        let T = d.exec(a);
        if (this.resumingScanAtSamePosition() && !(T && T.index === this.lastIndex)) {
          const O = this.getMatcher(0);
          O.lastIndex = this.lastIndex + 1, T = O.exec(a);
        }
        return T && (this.regexIndex += T.position + 1, this.regexIndex === this.count && this.considerAll()), T;
      }
    }
    function y(o) {
      const a = new g();
      return o.contains.forEach((d) => a.addRule(d.begin, { rule: d, type: "begin" })), o.terminatorEnd && a.addRule(o.terminatorEnd, { type: "end" }), o.illegal && a.addRule(o.illegal, { type: "illegal" }), a;
    }
    function w(o, a) {
      const d = (
        /** @type CompiledMode */
        o
      );
      if (o.isCompiled) return d;
      [
        un,
        // do this early so compiler extensions generally don't have to worry about
        // the distinction between match/begin
        pn,
        On,
        En
      ].forEach((O) => O(o, a)), e.compilerExtensions.forEach((O) => O(o, a)), o.__beforeBegin = null, [
        fn,
        // do this later so compiler extensions that come earlier have access to the
        // raw array if they wanted to perhaps manipulate it, etc.
        bn,
        // default to 1 relevance if not specified
        hn
      ].forEach((O) => O(o, a)), o.isCompiled = !0;
      let T = null;
      return typeof o.keywords == "object" && o.keywords.$pattern && (o.keywords = Object.assign({}, o.keywords), T = o.keywords.$pattern, delete o.keywords.$pattern), T = T || /\w+/, o.keywords && (o.keywords = Ue(o.keywords, e.case_insensitive)), d.keywordPatternRe = n(T, !0), a && (o.begin || (o.begin = /\B|\b/), d.beginRe = n(d.begin), !o.end && !o.endsWithParent && (o.end = /\B|\b/), o.end && (d.endRe = n(d.end)), d.terminatorEnd = D(d.end) || "", o.endsWithParent && a.terminatorEnd && (d.terminatorEnd += (o.end ? "|" : "") + a.terminatorEnd)), o.illegal && (d.illegalRe = n(
        /** @type {RegExp | string} */
        o.illegal
      )), o.contains || (o.contains = []), o.contains = [].concat(...o.contains.map(function(O) {
        return kn(O === "self" ? o : O);
      })), o.contains.forEach(function(O) {
        w(
          /** @type Mode */
          O,
          d
        );
      }), o.starts && w(o.starts, a), d.matcher = y(d), d;
    }
    if (e.compilerExtensions || (e.compilerExtensions = []), e.contains && e.contains.includes("self"))
      throw new Error("ERR: contains `self` is not supported at the top-level of a language.  See documentation.");
    return e.classNameAliases = b(e.classNameAliases || {}), w(
      /** @type Mode */
      e
    );
  }
  function He(e) {
    return e ? e.endsWithParent || He(e.starts) : !1;
  }
  function kn(e) {
    return e.variants && !e.cachedVariants && (e.cachedVariants = e.variants.map(function(n) {
      return b(e, { variants: null }, n);
    })), e.cachedVariants ? e.cachedVariants : He(e) ? b(e, { starts: e.starts ? b(e.starts) : null }) : Object.isFrozen(e) ? b(e) : e;
  }
  var Tn = "11.11.1";
  class An extends Error {
    constructor(n, i) {
      super(n), this.name = "HTMLInjectionError", this.html = i;
    }
  }
  const Me = E, Ke = b, Fe = /* @__PURE__ */ Symbol("nomatch"), Mn = 7, Ze = function(e) {
    const n = /* @__PURE__ */ Object.create(null), i = /* @__PURE__ */ Object.create(null), g = [];
    let y = !0;
    const w = "Could not find the language '{}', did you forget to load/include a language module?", o = { disableAutodetect: !0, name: "Plain text", contains: [] };
    let a = {
      ignoreUnescapedHTML: !1,
      throwUnescapedHTML: !1,
      noHighlightRe: /^(no-?highlight)$/i,
      languageDetectRe: /\blang(?:uage)?-([\w-]+)\b/i,
      classPrefix: "hljs-",
      cssSelector: "pre code",
      languages: null,
      // beta configuration options, subject to change, welcome to discuss
      // https://github.com/highlightjs/highlight.js/issues/1086
      __emitter: U
    };
    function d(t) {
      return a.noHighlightRe.test(t);
    }
    function T(t) {
      let l = t.className + " ";
      l += t.parentNode ? t.parentNode.className : "";
      const h = a.languageDetectRe.exec(l);
      if (h) {
        const _ = W(h[1]);
        return _ || ($e(w.replace("{}", h[1])), $e("Falling back to no-highlight mode for this block.", t)), _ ? h[1] : "no-highlight";
      }
      return l.split(/\s+/).find((_) => d(_) || W(_));
    }
    function O(t, l, h) {
      let _ = "", k = "";
      typeof l == "object" ? (_ = t, h = l.ignoreIllegals, k = l.language) : (se("10.7.0", "highlight(lang, code, ...args) has been deprecated."), se("10.7.0", `Please use highlight(code, options) instead.
https://github.com/highlightjs/highlight.js/issues/2277`), k = t, _ = l), h === void 0 && (h = !0);
      const G = {
        code: _,
        language: k
      };
      ye("before:highlight", G);
      const X = G.result ? G.result : de(G.language, G.code, h);
      return X.code = G.code, ye("after:highlight", X), X;
    }
    function de(t, l, h, _) {
      const k = /* @__PURE__ */ Object.create(null);
      function G(r, c) {
        return r.keywords[c];
      }
      function X() {
        if (!u.keywords) {
          R.addText(N);
          return;
        }
        let r = 0;
        u.keywordPatternRe.lastIndex = 0;
        let c = u.keywordPatternRe.exec(N), p = "";
        for (; c; ) {
          p += N.substring(r, c.index);
          const m = K.case_insensitive ? c[0].toLowerCase() : c[0], I = G(u, m);
          if (I) {
            const [Z, Wn] = I;
            if (R.addText(p), p = "", k[m] = (k[m] || 0) + 1, k[m] <= Mn && (Oe += Wn), Z.startsWith("_"))
              p += c[0];
            else {
              const Xn = K.classNameAliases[Z] || Z;
              H(c[0], Xn);
            }
          } else
            p += c[0];
          r = u.keywordPatternRe.lastIndex, c = u.keywordPatternRe.exec(N);
        }
        p += N.substring(r), R.addText(p);
      }
      function we() {
        if (N === "") return;
        let r = null;
        if (typeof u.subLanguage == "string") {
          if (!n[u.subLanguage]) {
            R.addText(N);
            return;
          }
          r = de(u.subLanguage, N, !0, Je[u.subLanguage]), Je[u.subLanguage] = /** @type {CompiledMode} */
          r._top;
        } else
          r = Ie(N, u.subLanguage.length ? u.subLanguage : null);
        u.relevance > 0 && (Oe += r.relevance), R.__addSublanguage(r._emitter, r.language);
      }
      function P() {
        u.subLanguage != null ? we() : X(), N = "";
      }
      function H(r, c) {
        r !== "" && (R.startScope(c), R.addText(r), R.endScope());
      }
      function Ye(r, c) {
        let p = 1;
        const m = c.length - 1;
        for (; p <= m; ) {
          if (!r._emit[p]) {
            p++;
            continue;
          }
          const I = K.classNameAliases[r[p]] || r[p], Z = c[p];
          I ? H(Z, I) : (N = Z, X(), N = ""), p++;
        }
      }
      function qe(r, c) {
        return r.scope && typeof r.scope == "string" && R.openNode(K.classNameAliases[r.scope] || r.scope), r.beginScope && (r.beginScope._wrap ? (H(N, K.classNameAliases[r.beginScope._wrap] || r.beginScope._wrap), N = "") : r.beginScope._multi && (Ye(r.beginScope, c), N = "")), u = Object.create(r, { parent: { value: u } }), u;
      }
      function Qe(r, c, p) {
        let m = ge(r.endRe, p);
        if (m) {
          if (r["on:end"]) {
            const I = new f(r);
            r["on:end"](c, I), I.isMatchIgnored && (m = !1);
          }
          if (m) {
            for (; r.endsParent && r.parent; )
              r = r.parent;
            return r;
          }
        }
        if (r.endsWithParent)
          return Qe(r.parent, c, p);
      }
      function Hn(r) {
        return u.matcher.regexIndex === 0 ? (N += r[0], 1) : (Be = !0, 0);
      }
      function Kn(r) {
        const c = r[0], p = r.rule, m = new f(p), I = [p.__beforeBegin, p["on:begin"]];
        for (const Z of I)
          if (Z && (Z(r, m), m.isMatchIgnored))
            return Hn(c);
        return p.skip ? N += c : (p.excludeBegin && (N += c), P(), !p.returnBegin && !p.excludeBegin && (N = c)), qe(p, r), p.returnBegin ? 0 : c.length;
      }
      function Fn(r) {
        const c = r[0], p = l.substring(r.index), m = Qe(u, r, p);
        if (!m)
          return Fe;
        const I = u;
        u.endScope && u.endScope._wrap ? (P(), H(c, u.endScope._wrap)) : u.endScope && u.endScope._multi ? (P(), Ye(u.endScope, r)) : I.skip ? N += c : (I.returnEnd || I.excludeEnd || (N += c), P(), I.excludeEnd && (N = c));
        do
          u.scope && R.closeNode(), !u.skip && !u.subLanguage && (Oe += u.relevance), u = u.parent;
        while (u !== m.parent);
        return m.starts && qe(m.starts, r), I.returnEnd ? 0 : c.length;
      }
      function Zn() {
        const r = [];
        for (let c = u; c !== K; c = c.parent)
          c.scope && r.unshift(c.scope);
        r.forEach((c) => R.openNode(c));
      }
      let ve = {};
      function Ve(r, c) {
        const p = c && c[0];
        if (N += r, p == null)
          return P(), 0;
        if (ve.type === "begin" && c.type === "end" && ve.index === c.index && p === "") {
          if (N += l.slice(c.index, c.index + 1), !y) {
            const m = new Error(`0 width match regex (${t})`);
            throw m.languageName = t, m.badRule = ve.rule, m;
          }
          return 1;
        }
        if (ve = c, c.type === "begin")
          return Kn(c);
        if (c.type === "illegal" && !h) {
          const m = new Error('Illegal lexeme "' + p + '" for mode "' + (u.scope || "<unnamed>") + '"');
          throw m.mode = u, m;
        } else if (c.type === "end") {
          const m = Fn(c);
          if (m !== Fe)
            return m;
        }
        if (c.type === "illegal" && p === "")
          return N += `
`, 1;
        if (De > 1e5 && De > c.index * 3)
          throw new Error("potential infinite loop, way more iterations than matches");
        return N += p, p.length;
      }
      const K = W(t);
      if (!K)
        throw ee(w.replace("{}", t)), new Error('Unknown language: "' + t + '"');
      const jn = Sn(K);
      let Ce = "", u = _ || jn;
      const Je = {}, R = new a.__emitter(a);
      Zn();
      let N = "", Oe = 0, ne = 0, De = 0, Be = !1;
      try {
        if (K.__emitTokens)
          K.__emitTokens(l, R);
        else {
          for (u.matcher.considerAll(); ; ) {
            De++, Be ? Be = !1 : u.matcher.considerAll(), u.matcher.lastIndex = ne;
            const r = u.matcher.exec(l);
            if (!r) break;
            const c = l.substring(ne, r.index), p = Ve(c, r);
            ne = r.index + p;
          }
          Ve(l.substring(ne));
        }
        return R.finalize(), Ce = R.toHTML(), {
          language: t,
          value: Ce,
          relevance: Oe,
          illegal: !1,
          _emitter: R,
          _top: u
        };
      } catch (r) {
        if (r.message && r.message.includes("Illegal"))
          return {
            language: t,
            value: Me(l),
            illegal: !0,
            relevance: 0,
            _illegalBy: {
              message: r.message,
              index: ne,
              context: l.slice(ne - 100, ne + 100),
              mode: r.mode,
              resultSoFar: Ce
            },
            _emitter: R
          };
        if (y)
          return {
            language: t,
            value: Me(l),
            illegal: !1,
            relevance: 0,
            errorRaised: r,
            _emitter: R,
            _top: u
          };
        throw r;
      }
    }
    function Re(t) {
      const l = {
        value: Me(t),
        illegal: !1,
        relevance: 0,
        _top: o,
        _emitter: new a.__emitter(a)
      };
      return l._emitter.addText(t), l;
    }
    function Ie(t, l) {
      l = l || a.languages || Object.keys(n);
      const h = Re(t), _ = l.filter(W).filter(Xe).map(
        (P) => de(P, t, !1)
      );
      _.unshift(h);
      const k = _.sort((P, H) => {
        if (P.relevance !== H.relevance) return H.relevance - P.relevance;
        if (P.language && H.language) {
          if (W(P.language).supersetOf === H.language)
            return 1;
          if (W(H.language).supersetOf === P.language)
            return -1;
        }
        return 0;
      }), [G, X] = k, we = G;
      return we.secondBest = X, we;
    }
    function Rn(t, l, h) {
      const _ = l && i[l] || h;
      t.classList.add("hljs"), t.classList.add(`language-${_}`);
    }
    function Le(t) {
      let l = null;
      const h = T(t);
      if (d(h)) return;
      if (ye(
        "before:highlightElement",
        { el: t, language: h }
      ), t.dataset.highlighted) {
        console.log("Element previously highlighted. To highlight again, first unset `dataset.highlighted`.", t);
        return;
      }
      if (t.children.length > 0 && (a.ignoreUnescapedHTML || (console.warn("One of your code blocks includes unescaped HTML. This is a potentially serious security risk."), console.warn("https://github.com/highlightjs/highlight.js/wiki/security"), console.warn("The element with unescaped HTML:"), console.warn(t)), a.throwUnescapedHTML))
        throw new An(
          "One of your code blocks includes unescaped HTML.",
          t.innerHTML
        );
      l = t;
      const _ = l.textContent, k = h ? O(_, { language: h, ignoreIllegals: !0 }) : Ie(_);
      t.innerHTML = k.value, t.dataset.highlighted = "yes", Rn(t, h, k.language), t.result = {
        language: k.language,
        // TODO: remove with version 11.0
        re: k.relevance,
        relevance: k.relevance
      }, k.secondBest && (t.secondBest = {
        language: k.secondBest.language,
        relevance: k.secondBest.relevance
      }), ye("after:highlightElement", { el: t, result: k, text: _ });
    }
    function In(t) {
      a = Ke(a, t);
    }
    const Ln = () => {
      xe(), se("10.6.0", "initHighlighting() deprecated.  Use highlightAll() now.");
    };
    function Cn() {
      xe(), se("10.6.0", "initHighlightingOnLoad() deprecated.  Use highlightAll() now.");
    }
    let je = !1;
    function xe() {
      function t() {
        xe();
      }
      if (document.readyState === "loading") {
        je || window.addEventListener("DOMContentLoaded", t, !1), je = !0;
        return;
      }
      document.querySelectorAll(a.cssSelector).forEach(Le);
    }
    function Dn(t, l) {
      let h = null;
      try {
        h = l(e);
      } catch (_) {
        if (ee("Language definition for '{}' could not be registered.".replace("{}", t)), y)
          ee(_);
        else
          throw _;
        h = o;
      }
      h.name || (h.name = t), n[t] = h, h.rawDefinition = l.bind(null, e), h.aliases && We(h.aliases, { languageName: t });
    }
    function Bn(t) {
      delete n[t];
      for (const l of Object.keys(i))
        i[l] === t && delete i[l];
    }
    function Pn() {
      return Object.keys(n);
    }
    function W(t) {
      return t = (t || "").toLowerCase(), n[t] || n[i[t]];
    }
    function We(t, { languageName: l }) {
      typeof t == "string" && (t = [t]), t.forEach((h) => {
        i[h.toLowerCase()] = l;
      });
    }
    function Xe(t) {
      const l = W(t);
      return l && !l.disableAutodetect;
    }
    function Un(t) {
      t["before:highlightBlock"] && !t["before:highlightElement"] && (t["before:highlightElement"] = (l) => {
        t["before:highlightBlock"](
          Object.assign({ block: l.el }, l)
        );
      }), t["after:highlightBlock"] && !t["after:highlightElement"] && (t["after:highlightElement"] = (l) => {
        t["after:highlightBlock"](
          Object.assign({ block: l.el }, l)
        );
      });
    }
    function zn(t) {
      Un(t), g.push(t);
    }
    function $n(t) {
      const l = g.indexOf(t);
      l !== -1 && g.splice(l, 1);
    }
    function ye(t, l) {
      const h = t;
      g.forEach(function(_) {
        _[h] && _[h](l);
      });
    }
    function Gn(t) {
      return se("10.7.0", "highlightBlock will be removed entirely in v12.0"), se("10.7.0", "Please use highlightElement now."), Le(t);
    }
    Object.assign(e, {
      highlight: O,
      highlightAuto: Ie,
      highlightAll: xe,
      highlightElement: Le,
      // TODO: Remove with v12 API
      highlightBlock: Gn,
      configure: In,
      initHighlighting: Ln,
      initHighlightingOnLoad: Cn,
      registerLanguage: Dn,
      unregisterLanguage: Bn,
      listLanguages: Pn,
      getLanguage: W,
      registerAliases: We,
      autoDetection: Xe,
      inherit: Ke,
      addPlugin: zn,
      removePlugin: $n
    }), e.debugMode = function() {
      y = !1;
    }, e.safeMode = function() {
      y = !0;
    }, e.versionString = Tn, e.regex = {
      concat: $,
      lookahead: B,
      either: Y,
      optional: oe,
      anyNumberOfTimes: z
    };
    for (const t in _e)
      typeof _e[t] == "object" && s(_e[t]);
    return Object.assign(e, _e), e;
  }, ae = Ze({});
  return ae.newInstance = () => Ze({}), Pe = ae, ae.HighlightJS = ae, ae.default = ae, Pe;
}
var Qn = /* @__PURE__ */ qn();
const F = /* @__PURE__ */ Yn(Qn);
function Vn(s) {
  const f = "\\[=*\\[", E = "\\]=*\\]", b = {
    begin: f,
    end: E,
    contains: ["self"]
  }, x = [
    s.COMMENT("--(?!" + f + ")", "$"),
    s.COMMENT(
      "--" + f,
      E,
      {
        contains: [b],
        relevance: 10
      }
    )
  ];
  return {
    name: "Lua",
    aliases: ["pluto"],
    keywords: {
      $pattern: s.UNDERSCORE_IDENT_RE,
      literal: "true false nil",
      keyword: "and break do else elseif end for goto if in local not or repeat return then until while",
      built_in: (
        // Metatags and globals:
        "_G _ENV _VERSION __index __newindex __mode __call __metatable __tostring __len __gc __add __sub __mul __div __mod __pow __concat __unm __eq __lt __le assert collectgarbage dofile error getfenv getmetatable ipairs load loadfile loadstring module next pairs pcall print rawequal rawget rawset require select setfenv setmetatable tonumber tostring type unpack xpcall arg self coroutine resume yield status wrap create running debug getupvalue debug sethook getmetatable gethook setmetatable setlocal traceback setfenv getinfo setupvalue getlocal getregistry getfenv io lines write close flush open output type read stderr stdin input stdout popen tmpfile math log max acos huge ldexp pi cos tanh pow deg tan cosh sinh random randomseed frexp ceil floor rad abs sqrt modf asin min mod fmod log10 atan2 exp sin atan os exit setlocale date getenv difftime remove time clock tmpname rename execute package preload loadlib loaded loaders cpath config path seeall string sub upper len gfind rep find match char dump gmatch reverse byte format gsub lower table setn insert getn foreachi maxn foreach concat sort remove"
      )
    },
    contains: x.concat([
      {
        className: "function",
        beginKeywords: "function",
        end: "\\)",
        contains: [
          s.inherit(s.TITLE_MODE, { begin: "([_a-zA-Z]\\w*\\.)*([_a-zA-Z]\\w*:)?[_a-zA-Z]\\w*" }),
          {
            className: "params",
            begin: "\\(",
            endsWithParent: !0,
            contains: x
          }
        ].concat(x)
      },
      s.C_NUMBER_MODE,
      s.APOS_STRING_MODE,
      s.QUOTE_STRING_MODE,
      {
        className: "string",
        begin: f,
        end: E,
        contains: [b],
        relevance: 5
      }
    ])
  };
}
function Jn(s) {
  const f = s.regex, E = f.concat(/[\p{L}_]/u, f.optional(/[\p{L}0-9_.-]*:/u), /[\p{L}0-9_.-]*/u), b = /[\p{L}0-9._:-]+/u, x = {
    className: "symbol",
    begin: /&[a-z]+;|&#[0-9]+;|&#x[a-f0-9]+;/
  }, L = {
    begin: /\s/,
    contains: [
      {
        className: "keyword",
        begin: /#?[a-z_][a-z1-9_-]+/,
        illegal: /\n/
      }
    ]
  }, A = s.inherit(L, {
    begin: /\(/,
    end: /\)/
  }), S = s.inherit(s.APOS_STRING_MODE, { className: "string" }), C = s.inherit(s.QUOTE_STRING_MODE, { className: "string" }), v = {
    endsWithParent: !0,
    illegal: /</,
    relevance: 0,
    contains: [
      {
        className: "attr",
        begin: b,
        relevance: 0
      },
      {
        begin: /=\s*/,
        relevance: 0,
        contains: [
          {
            className: "string",
            endsParent: !0,
            variants: [
              {
                begin: /"/,
                end: /"/,
                contains: [x]
              },
              {
                begin: /'/,
                end: /'/,
                contains: [x]
              },
              { begin: /[^\s"'=<>`]+/ }
            ]
          }
        ]
      }
    ]
  };
  return {
    name: "HTML, XML",
    aliases: [
      "html",
      "xhtml",
      "rss",
      "atom",
      "xjb",
      "xsd",
      "xsl",
      "plist",
      "wsf",
      "svg"
    ],
    case_insensitive: !0,
    unicodeRegex: !0,
    contains: [
      {
        className: "meta",
        begin: /<![a-z]/,
        end: />/,
        relevance: 10,
        contains: [
          L,
          C,
          S,
          A,
          {
            begin: /\[/,
            end: /\]/,
            contains: [
              {
                className: "meta",
                begin: /<![a-z]/,
                end: />/,
                contains: [
                  L,
                  A,
                  C,
                  S
                ]
              }
            ]
          }
        ]
      },
      s.COMMENT(
        /<!--/,
        /-->/,
        { relevance: 10 }
      ),
      {
        begin: /<!\[CDATA\[/,
        end: /\]\]>/,
        relevance: 10
      },
      x,
      // xml processing instructions
      {
        className: "meta",
        end: /\?>/,
        variants: [
          {
            begin: /<\?xml/,
            relevance: 10,
            contains: [
              C
            ]
          },
          {
            begin: /<\?[a-z][a-z0-9]+/
          }
        ]
      },
      {
        className: "tag",
        /*
        The lookahead pattern (?=...) ensures that 'begin' only matches
        '<style' as a single word, followed by a whitespace or an
        ending bracket.
        */
        begin: /<style(?=\s|>)/,
        end: />/,
        keywords: { name: "style" },
        contains: [v],
        starts: {
          end: /<\/style>/,
          returnEnd: !0,
          subLanguage: [
            "css",
            "xml"
          ]
        }
      },
      {
        className: "tag",
        // See the comment in the <style tag about the lookahead pattern
        begin: /<script(?=\s|>)/,
        end: />/,
        keywords: { name: "script" },
        contains: [v],
        starts: {
          end: /<\/script>/,
          returnEnd: !0,
          subLanguage: [
            "javascript",
            "handlebars",
            "xml"
          ]
        }
      },
      // we need this for now for jSX
      {
        className: "tag",
        begin: /<>|<\/>/
      },
      // open tag
      {
        className: "tag",
        begin: f.concat(
          /</,
          f.lookahead(f.concat(
            E,
            // <tag/>
            // <tag>
            // <tag ...
            f.either(/\/>/, />/, /\s/)
          ))
        ),
        end: /\/?>/,
        contains: [
          {
            className: "name",
            begin: E,
            relevance: 0,
            starts: v
          }
        ]
      },
      // close tag
      {
        className: "tag",
        begin: f.concat(
          /<\//,
          f.lookahead(f.concat(
            E,
            />/
          ))
        ),
        contains: [
          {
            className: "name",
            begin: E,
            relevance: 0
          },
          {
            begin: />/,
            relevance: 0,
            endsParent: !0
          }
        ]
      }
    ]
  };
}
function et(s) {
  const f = s.regex, E = {
    begin: /<\/?[A-Za-z_]/,
    end: ">",
    subLanguage: "xml",
    relevance: 0
  }, b = {
    begin: "^[-\\*]{3,}",
    end: "$"
  }, x = {
    className: "code",
    variants: [
      // TODO: fix to allow these to work with sublanguage also
      { begin: "(`{3,})[^`](.|\\n)*?\\1`*[ ]*" },
      { begin: "(~{3,})[^~](.|\\n)*?\\1~*[ ]*" },
      // needed to allow markdown as a sublanguage to work
      {
        begin: "```",
        end: "```+[ ]*$"
      },
      {
        begin: "~~~",
        end: "~~~+[ ]*$"
      },
      { begin: "`.+?`" },
      {
        begin: "(?=^( {4}|\\t))",
        // use contains to gobble up multiple lines to allow the block to be whatever size
        // but only have a single open/close tag vs one per line
        contains: [
          {
            begin: "^( {4}|\\t)",
            end: "(\\n)$"
          }
        ],
        relevance: 0
      }
    ]
  }, L = {
    className: "bullet",
    begin: "^[ 	]*([*+-]|(\\d+\\.))(?=\\s+)",
    end: "\\s+",
    excludeEnd: !0
  }, A = {
    begin: /^\[[^\n]+\]:/,
    returnBegin: !0,
    contains: [
      {
        className: "symbol",
        begin: /\[/,
        end: /\]/,
        excludeBegin: !0,
        excludeEnd: !0
      },
      {
        className: "link",
        begin: /:\s*/,
        end: /$/,
        excludeBegin: !0
      }
    ]
  }, S = /[A-Za-z][A-Za-z0-9+.-]*/, C = {
    variants: [
      // too much like nested array access in so many languages
      // to have any real relevance
      {
        begin: /\[.+?\]\[.*?\]/,
        relevance: 0
      },
      // popular internet URLs
      {
        begin: /\[.+?\]\(((data|javascript|mailto):|(?:http|ftp)s?:\/\/).*?\)/,
        relevance: 2
      },
      {
        begin: f.concat(/\[.+?\]\(/, S, /:\/\/.*?\)/),
        relevance: 2
      },
      // relative urls
      {
        begin: /\[.+?\]\([./?&#].*?\)/,
        relevance: 1
      },
      // whatever else, lower relevance (might not be a link at all)
      {
        begin: /\[.*?\]\(.*?\)/,
        relevance: 0
      }
    ],
    returnBegin: !0,
    contains: [
      {
        // empty strings for alt or link text
        match: /\[(?=\])/
      },
      {
        className: "string",
        relevance: 0,
        begin: "\\[",
        end: "\\]",
        excludeBegin: !0,
        returnEnd: !0
      },
      {
        className: "link",
        relevance: 0,
        begin: "\\]\\(",
        end: "\\)",
        excludeBegin: !0,
        excludeEnd: !0
      },
      {
        className: "symbol",
        relevance: 0,
        begin: "\\]\\[",
        end: "\\]",
        excludeBegin: !0,
        excludeEnd: !0
      }
    ]
  }, v = {
    className: "strong",
    contains: [],
    // defined later
    variants: [
      {
        begin: /_{2}(?!\s)/,
        end: /_{2}/
      },
      {
        begin: /\*{2}(?!\s)/,
        end: /\*{2}/
      }
    ]
  }, U = {
    className: "emphasis",
    contains: [],
    // defined later
    variants: [
      {
        begin: /\*(?![*\s])/,
        end: /\*/
      },
      {
        begin: /_(?![_\s])/,
        end: /_/,
        relevance: 0
      }
    ]
  }, D = s.inherit(v, { contains: [] }), B = s.inherit(U, { contains: [] });
  v.contains.push(B), U.contains.push(D);
  let z = [
    E,
    C
  ];
  return [
    v,
    U,
    D,
    B
  ].forEach((Y) => {
    Y.contains = Y.contains.concat(z);
  }), z = z.concat(v, U), {
    name: "Markdown",
    aliases: [
      "md",
      "mkdown",
      "mkd"
    ],
    contains: [
      {
        className: "section",
        variants: [
          {
            begin: "^#{1,6}",
            end: "$",
            contains: z
          },
          {
            begin: "(?=^.+?\\n[=-]{2,}$)",
            contains: [
              { begin: "^[=-]*$" },
              {
                begin: "^",
                end: "\\n",
                contains: z
              }
            ]
          }
        ]
      },
      E,
      L,
      v,
      U,
      {
        className: "quote",
        begin: "^>\\s+",
        contains: z,
        end: "$"
      },
      x,
      b,
      C,
      A,
      {
        //https://spec.commonmark.org/0.31.2/#entity-references
        scope: "literal",
        match: /&([a-zA-Z0-9]+|#[0-9]{1,7}|#[Xx][0-9a-fA-F]{1,6});/
      }
    ]
  };
}
const nt = (s) => ({
  IMPORTANT: {
    scope: "meta",
    begin: "!important"
  },
  BLOCK_COMMENT: s.C_BLOCK_COMMENT_MODE,
  HEXCOLOR: {
    scope: "number",
    begin: /#(([0-9a-fA-F]{3,4})|(([0-9a-fA-F]{2}){3,4}))\b/
  },
  FUNCTION_DISPATCH: {
    className: "built_in",
    begin: /[\w-]+(?=\()/
  },
  ATTRIBUTE_SELECTOR_MODE: {
    scope: "selector-attr",
    begin: /\[/,
    end: /\]/,
    illegal: "$",
    contains: [
      s.APOS_STRING_MODE,
      s.QUOTE_STRING_MODE
    ]
  },
  CSS_NUMBER_MODE: {
    scope: "number",
    begin: s.NUMBER_RE + "(%|em|ex|ch|rem|vw|vh|vmin|vmax|cm|mm|in|pt|pc|px|deg|grad|rad|turn|s|ms|Hz|kHz|dpi|dpcm|dppx)?",
    relevance: 0
  },
  CSS_VARIABLE: {
    className: "attr",
    begin: /--[A-Za-z_][A-Za-z0-9_-]*/
  }
}), tt = [
  "a",
  "abbr",
  "address",
  "article",
  "aside",
  "audio",
  "b",
  "blockquote",
  "body",
  "button",
  "canvas",
  "caption",
  "cite",
  "code",
  "dd",
  "del",
  "details",
  "dfn",
  "div",
  "dl",
  "dt",
  "em",
  "fieldset",
  "figcaption",
  "figure",
  "footer",
  "form",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "header",
  "hgroup",
  "html",
  "i",
  "iframe",
  "img",
  "input",
  "ins",
  "kbd",
  "label",
  "legend",
  "li",
  "main",
  "mark",
  "menu",
  "nav",
  "object",
  "ol",
  "optgroup",
  "option",
  "p",
  "picture",
  "q",
  "quote",
  "samp",
  "section",
  "select",
  "source",
  "span",
  "strong",
  "summary",
  "sup",
  "table",
  "tbody",
  "td",
  "textarea",
  "tfoot",
  "th",
  "thead",
  "time",
  "tr",
  "ul",
  "var",
  "video"
], it = [
  "defs",
  "g",
  "marker",
  "mask",
  "pattern",
  "svg",
  "switch",
  "symbol",
  "feBlend",
  "feColorMatrix",
  "feComponentTransfer",
  "feComposite",
  "feConvolveMatrix",
  "feDiffuseLighting",
  "feDisplacementMap",
  "feFlood",
  "feGaussianBlur",
  "feImage",
  "feMerge",
  "feMorphology",
  "feOffset",
  "feSpecularLighting",
  "feTile",
  "feTurbulence",
  "linearGradient",
  "radialGradient",
  "stop",
  "circle",
  "ellipse",
  "image",
  "line",
  "path",
  "polygon",
  "polyline",
  "rect",
  "text",
  "use",
  "textPath",
  "tspan",
  "foreignObject",
  "clipPath"
], rt = [
  ...tt,
  ...it
], st = [
  "any-hover",
  "any-pointer",
  "aspect-ratio",
  "color",
  "color-gamut",
  "color-index",
  "device-aspect-ratio",
  "device-height",
  "device-width",
  "display-mode",
  "forced-colors",
  "grid",
  "height",
  "hover",
  "inverted-colors",
  "monochrome",
  "orientation",
  "overflow-block",
  "overflow-inline",
  "pointer",
  "prefers-color-scheme",
  "prefers-contrast",
  "prefers-reduced-motion",
  "prefers-reduced-transparency",
  "resolution",
  "scan",
  "scripting",
  "update",
  "width",
  // TODO: find a better solution?
  "min-width",
  "max-width",
  "min-height",
  "max-height"
].sort().reverse(), at = [
  "active",
  "any-link",
  "blank",
  "checked",
  "current",
  "default",
  "defined",
  "dir",
  // dir()
  "disabled",
  "drop",
  "empty",
  "enabled",
  "first",
  "first-child",
  "first-of-type",
  "fullscreen",
  "future",
  "focus",
  "focus-visible",
  "focus-within",
  "has",
  // has()
  "host",
  // host or host()
  "host-context",
  // host-context()
  "hover",
  "indeterminate",
  "in-range",
  "invalid",
  "is",
  // is()
  "lang",
  // lang()
  "last-child",
  "last-of-type",
  "left",
  "link",
  "local-link",
  "not",
  // not()
  "nth-child",
  // nth-child()
  "nth-col",
  // nth-col()
  "nth-last-child",
  // nth-last-child()
  "nth-last-col",
  // nth-last-col()
  "nth-last-of-type",
  //nth-last-of-type()
  "nth-of-type",
  //nth-of-type()
  "only-child",
  "only-of-type",
  "optional",
  "out-of-range",
  "past",
  "placeholder-shown",
  "read-only",
  "read-write",
  "required",
  "right",
  "root",
  "scope",
  "target",
  "target-within",
  "user-invalid",
  "valid",
  "visited",
  "where"
  // where()
].sort().reverse(), ot = [
  "after",
  "backdrop",
  "before",
  "cue",
  "cue-region",
  "first-letter",
  "first-line",
  "grammar-error",
  "marker",
  "part",
  "placeholder",
  "selection",
  "slotted",
  "spelling-error"
].sort().reverse(), ct = [
  "accent-color",
  "align-content",
  "align-items",
  "align-self",
  "alignment-baseline",
  "all",
  "anchor-name",
  "animation",
  "animation-composition",
  "animation-delay",
  "animation-direction",
  "animation-duration",
  "animation-fill-mode",
  "animation-iteration-count",
  "animation-name",
  "animation-play-state",
  "animation-range",
  "animation-range-end",
  "animation-range-start",
  "animation-timeline",
  "animation-timing-function",
  "appearance",
  "aspect-ratio",
  "backdrop-filter",
  "backface-visibility",
  "background",
  "background-attachment",
  "background-blend-mode",
  "background-clip",
  "background-color",
  "background-image",
  "background-origin",
  "background-position",
  "background-position-x",
  "background-position-y",
  "background-repeat",
  "background-size",
  "baseline-shift",
  "block-size",
  "border",
  "border-block",
  "border-block-color",
  "border-block-end",
  "border-block-end-color",
  "border-block-end-style",
  "border-block-end-width",
  "border-block-start",
  "border-block-start-color",
  "border-block-start-style",
  "border-block-start-width",
  "border-block-style",
  "border-block-width",
  "border-bottom",
  "border-bottom-color",
  "border-bottom-left-radius",
  "border-bottom-right-radius",
  "border-bottom-style",
  "border-bottom-width",
  "border-collapse",
  "border-color",
  "border-end-end-radius",
  "border-end-start-radius",
  "border-image",
  "border-image-outset",
  "border-image-repeat",
  "border-image-slice",
  "border-image-source",
  "border-image-width",
  "border-inline",
  "border-inline-color",
  "border-inline-end",
  "border-inline-end-color",
  "border-inline-end-style",
  "border-inline-end-width",
  "border-inline-start",
  "border-inline-start-color",
  "border-inline-start-style",
  "border-inline-start-width",
  "border-inline-style",
  "border-inline-width",
  "border-left",
  "border-left-color",
  "border-left-style",
  "border-left-width",
  "border-radius",
  "border-right",
  "border-right-color",
  "border-right-style",
  "border-right-width",
  "border-spacing",
  "border-start-end-radius",
  "border-start-start-radius",
  "border-style",
  "border-top",
  "border-top-color",
  "border-top-left-radius",
  "border-top-right-radius",
  "border-top-style",
  "border-top-width",
  "border-width",
  "bottom",
  "box-align",
  "box-decoration-break",
  "box-direction",
  "box-flex",
  "box-flex-group",
  "box-lines",
  "box-ordinal-group",
  "box-orient",
  "box-pack",
  "box-shadow",
  "box-sizing",
  "break-after",
  "break-before",
  "break-inside",
  "caption-side",
  "caret-color",
  "clear",
  "clip",
  "clip-path",
  "clip-rule",
  "color",
  "color-interpolation",
  "color-interpolation-filters",
  "color-profile",
  "color-rendering",
  "color-scheme",
  "column-count",
  "column-fill",
  "column-gap",
  "column-rule",
  "column-rule-color",
  "column-rule-style",
  "column-rule-width",
  "column-span",
  "column-width",
  "columns",
  "contain",
  "contain-intrinsic-block-size",
  "contain-intrinsic-height",
  "contain-intrinsic-inline-size",
  "contain-intrinsic-size",
  "contain-intrinsic-width",
  "container",
  "container-name",
  "container-type",
  "content",
  "content-visibility",
  "counter-increment",
  "counter-reset",
  "counter-set",
  "cue",
  "cue-after",
  "cue-before",
  "cursor",
  "cx",
  "cy",
  "direction",
  "display",
  "dominant-baseline",
  "empty-cells",
  "enable-background",
  "field-sizing",
  "fill",
  "fill-opacity",
  "fill-rule",
  "filter",
  "flex",
  "flex-basis",
  "flex-direction",
  "flex-flow",
  "flex-grow",
  "flex-shrink",
  "flex-wrap",
  "float",
  "flood-color",
  "flood-opacity",
  "flow",
  "font",
  "font-display",
  "font-family",
  "font-feature-settings",
  "font-kerning",
  "font-language-override",
  "font-optical-sizing",
  "font-palette",
  "font-size",
  "font-size-adjust",
  "font-smooth",
  "font-smoothing",
  "font-stretch",
  "font-style",
  "font-synthesis",
  "font-synthesis-position",
  "font-synthesis-small-caps",
  "font-synthesis-style",
  "font-synthesis-weight",
  "font-variant",
  "font-variant-alternates",
  "font-variant-caps",
  "font-variant-east-asian",
  "font-variant-emoji",
  "font-variant-ligatures",
  "font-variant-numeric",
  "font-variant-position",
  "font-variation-settings",
  "font-weight",
  "forced-color-adjust",
  "gap",
  "glyph-orientation-horizontal",
  "glyph-orientation-vertical",
  "grid",
  "grid-area",
  "grid-auto-columns",
  "grid-auto-flow",
  "grid-auto-rows",
  "grid-column",
  "grid-column-end",
  "grid-column-start",
  "grid-gap",
  "grid-row",
  "grid-row-end",
  "grid-row-start",
  "grid-template",
  "grid-template-areas",
  "grid-template-columns",
  "grid-template-rows",
  "hanging-punctuation",
  "height",
  "hyphenate-character",
  "hyphenate-limit-chars",
  "hyphens",
  "icon",
  "image-orientation",
  "image-rendering",
  "image-resolution",
  "ime-mode",
  "initial-letter",
  "initial-letter-align",
  "inline-size",
  "inset",
  "inset-area",
  "inset-block",
  "inset-block-end",
  "inset-block-start",
  "inset-inline",
  "inset-inline-end",
  "inset-inline-start",
  "isolation",
  "justify-content",
  "justify-items",
  "justify-self",
  "kerning",
  "left",
  "letter-spacing",
  "lighting-color",
  "line-break",
  "line-height",
  "line-height-step",
  "list-style",
  "list-style-image",
  "list-style-position",
  "list-style-type",
  "margin",
  "margin-block",
  "margin-block-end",
  "margin-block-start",
  "margin-bottom",
  "margin-inline",
  "margin-inline-end",
  "margin-inline-start",
  "margin-left",
  "margin-right",
  "margin-top",
  "margin-trim",
  "marker",
  "marker-end",
  "marker-mid",
  "marker-start",
  "marks",
  "mask",
  "mask-border",
  "mask-border-mode",
  "mask-border-outset",
  "mask-border-repeat",
  "mask-border-slice",
  "mask-border-source",
  "mask-border-width",
  "mask-clip",
  "mask-composite",
  "mask-image",
  "mask-mode",
  "mask-origin",
  "mask-position",
  "mask-repeat",
  "mask-size",
  "mask-type",
  "masonry-auto-flow",
  "math-depth",
  "math-shift",
  "math-style",
  "max-block-size",
  "max-height",
  "max-inline-size",
  "max-width",
  "min-block-size",
  "min-height",
  "min-inline-size",
  "min-width",
  "mix-blend-mode",
  "nav-down",
  "nav-index",
  "nav-left",
  "nav-right",
  "nav-up",
  "none",
  "normal",
  "object-fit",
  "object-position",
  "offset",
  "offset-anchor",
  "offset-distance",
  "offset-path",
  "offset-position",
  "offset-rotate",
  "opacity",
  "order",
  "orphans",
  "outline",
  "outline-color",
  "outline-offset",
  "outline-style",
  "outline-width",
  "overflow",
  "overflow-anchor",
  "overflow-block",
  "overflow-clip-margin",
  "overflow-inline",
  "overflow-wrap",
  "overflow-x",
  "overflow-y",
  "overlay",
  "overscroll-behavior",
  "overscroll-behavior-block",
  "overscroll-behavior-inline",
  "overscroll-behavior-x",
  "overscroll-behavior-y",
  "padding",
  "padding-block",
  "padding-block-end",
  "padding-block-start",
  "padding-bottom",
  "padding-inline",
  "padding-inline-end",
  "padding-inline-start",
  "padding-left",
  "padding-right",
  "padding-top",
  "page",
  "page-break-after",
  "page-break-before",
  "page-break-inside",
  "paint-order",
  "pause",
  "pause-after",
  "pause-before",
  "perspective",
  "perspective-origin",
  "place-content",
  "place-items",
  "place-self",
  "pointer-events",
  "position",
  "position-anchor",
  "position-visibility",
  "print-color-adjust",
  "quotes",
  "r",
  "resize",
  "rest",
  "rest-after",
  "rest-before",
  "right",
  "rotate",
  "row-gap",
  "ruby-align",
  "ruby-position",
  "scale",
  "scroll-behavior",
  "scroll-margin",
  "scroll-margin-block",
  "scroll-margin-block-end",
  "scroll-margin-block-start",
  "scroll-margin-bottom",
  "scroll-margin-inline",
  "scroll-margin-inline-end",
  "scroll-margin-inline-start",
  "scroll-margin-left",
  "scroll-margin-right",
  "scroll-margin-top",
  "scroll-padding",
  "scroll-padding-block",
  "scroll-padding-block-end",
  "scroll-padding-block-start",
  "scroll-padding-bottom",
  "scroll-padding-inline",
  "scroll-padding-inline-end",
  "scroll-padding-inline-start",
  "scroll-padding-left",
  "scroll-padding-right",
  "scroll-padding-top",
  "scroll-snap-align",
  "scroll-snap-stop",
  "scroll-snap-type",
  "scroll-timeline",
  "scroll-timeline-axis",
  "scroll-timeline-name",
  "scrollbar-color",
  "scrollbar-gutter",
  "scrollbar-width",
  "shape-image-threshold",
  "shape-margin",
  "shape-outside",
  "shape-rendering",
  "speak",
  "speak-as",
  "src",
  // @font-face
  "stop-color",
  "stop-opacity",
  "stroke",
  "stroke-dasharray",
  "stroke-dashoffset",
  "stroke-linecap",
  "stroke-linejoin",
  "stroke-miterlimit",
  "stroke-opacity",
  "stroke-width",
  "tab-size",
  "table-layout",
  "text-align",
  "text-align-all",
  "text-align-last",
  "text-anchor",
  "text-combine-upright",
  "text-decoration",
  "text-decoration-color",
  "text-decoration-line",
  "text-decoration-skip",
  "text-decoration-skip-ink",
  "text-decoration-style",
  "text-decoration-thickness",
  "text-emphasis",
  "text-emphasis-color",
  "text-emphasis-position",
  "text-emphasis-style",
  "text-indent",
  "text-justify",
  "text-orientation",
  "text-overflow",
  "text-rendering",
  "text-shadow",
  "text-size-adjust",
  "text-transform",
  "text-underline-offset",
  "text-underline-position",
  "text-wrap",
  "text-wrap-mode",
  "text-wrap-style",
  "timeline-scope",
  "top",
  "touch-action",
  "transform",
  "transform-box",
  "transform-origin",
  "transform-style",
  "transition",
  "transition-behavior",
  "transition-delay",
  "transition-duration",
  "transition-property",
  "transition-timing-function",
  "translate",
  "unicode-bidi",
  "user-modify",
  "user-select",
  "vector-effect",
  "vertical-align",
  "view-timeline",
  "view-timeline-axis",
  "view-timeline-inset",
  "view-timeline-name",
  "view-transition-name",
  "visibility",
  "voice-balance",
  "voice-duration",
  "voice-family",
  "voice-pitch",
  "voice-range",
  "voice-rate",
  "voice-stress",
  "voice-volume",
  "white-space",
  "white-space-collapse",
  "widows",
  "width",
  "will-change",
  "word-break",
  "word-spacing",
  "word-wrap",
  "writing-mode",
  "x",
  "y",
  "z-index",
  "zoom"
].sort().reverse();
function lt(s) {
  const f = s.regex, E = nt(s), b = { begin: /-(webkit|moz|ms|o)-(?=[a-z])/ }, x = "and or not only", L = /@-?\w[\w]*(-\w+)*/, A = "[a-zA-Z-][a-zA-Z0-9_-]*", S = [
    s.APOS_STRING_MODE,
    s.QUOTE_STRING_MODE
  ];
  return {
    name: "CSS",
    case_insensitive: !0,
    illegal: /[=|'\$]/,
    keywords: { keyframePosition: "from to" },
    classNameAliases: {
      // for visual continuity with `tag {}` and because we
      // don't have a great class for this?
      keyframePosition: "selector-tag"
    },
    contains: [
      E.BLOCK_COMMENT,
      b,
      // to recognize keyframe 40% etc which are outside the scope of our
      // attribute value mode
      E.CSS_NUMBER_MODE,
      {
        className: "selector-id",
        begin: /#[A-Za-z0-9_-]+/,
        relevance: 0
      },
      {
        className: "selector-class",
        begin: "\\." + A,
        relevance: 0
      },
      E.ATTRIBUTE_SELECTOR_MODE,
      {
        className: "selector-pseudo",
        variants: [
          { begin: ":(" + at.join("|") + ")" },
          { begin: ":(:)?(" + ot.join("|") + ")" }
        ]
      },
      // we may actually need this (12/2020)
      // { // pseudo-selector params
      //   begin: /\(/,
      //   end: /\)/,
      //   contains: [ hljs.CSS_NUMBER_MODE ]
      // },
      E.CSS_VARIABLE,
      {
        className: "attribute",
        begin: "\\b(" + ct.join("|") + ")\\b"
      },
      // attribute values
      {
        begin: /:/,
        end: /[;}{]/,
        contains: [
          E.BLOCK_COMMENT,
          E.HEXCOLOR,
          E.IMPORTANT,
          E.CSS_NUMBER_MODE,
          ...S,
          // needed to highlight these as strings and to avoid issues with
          // illegal characters that might be inside urls that would tigger the
          // languages illegal stack
          {
            begin: /(url|data-uri)\(/,
            end: /\)/,
            relevance: 0,
            // from keywords
            keywords: { built_in: "url data-uri" },
            contains: [
              ...S,
              {
                className: "string",
                // any character other than `)` as in `url()` will be the start
                // of a string, which ends with `)` (from the parent mode)
                begin: /[^)]/,
                endsWithParent: !0,
                excludeEnd: !0
              }
            ]
          },
          E.FUNCTION_DISPATCH
        ]
      },
      {
        begin: f.lookahead(/@/),
        end: "[{;]",
        relevance: 0,
        illegal: /:/,
        // break on Less variables @var: ...
        contains: [
          {
            className: "keyword",
            begin: L
          },
          {
            begin: /\s/,
            endsWithParent: !0,
            excludeEnd: !0,
            relevance: 0,
            keywords: {
              $pattern: /[a-z-]+/,
              keyword: x,
              attribute: st.join(" ")
            },
            contains: [
              {
                begin: /[a-z-]+(?=:)/,
                className: "attribute"
              },
              ...S,
              E.CSS_NUMBER_MODE
            ]
          }
        ]
      },
      {
        className: "selector-tag",
        begin: "\\b(" + rt.join("|") + ")\\b"
      }
    ]
  };
}
const nn = "[A-Za-z$_][0-9A-Za-z$_]*", dt = [
  "as",
  // for exports
  "in",
  "of",
  "if",
  "for",
  "while",
  "finally",
  "var",
  "new",
  "function",
  "do",
  "return",
  "void",
  "else",
  "break",
  "catch",
  "instanceof",
  "with",
  "throw",
  "case",
  "default",
  "try",
  "switch",
  "continue",
  "typeof",
  "delete",
  "let",
  "yield",
  "const",
  "class",
  // JS handles these with a special rule
  // "get",
  // "set",
  "debugger",
  "async",
  "await",
  "static",
  "import",
  "from",
  "export",
  "extends",
  // It's reached stage 3, which is "recommended for implementation":
  "using"
], gt = [
  "true",
  "false",
  "null",
  "undefined",
  "NaN",
  "Infinity"
], tn = [
  // Fundamental objects
  "Object",
  "Function",
  "Boolean",
  "Symbol",
  // numbers and dates
  "Math",
  "Date",
  "Number",
  "BigInt",
  // text
  "String",
  "RegExp",
  // Indexed collections
  "Array",
  "Float32Array",
  "Float64Array",
  "Int8Array",
  "Uint8Array",
  "Uint8ClampedArray",
  "Int16Array",
  "Int32Array",
  "Uint16Array",
  "Uint32Array",
  "BigInt64Array",
  "BigUint64Array",
  // Keyed collections
  "Set",
  "Map",
  "WeakSet",
  "WeakMap",
  // Structured data
  "ArrayBuffer",
  "SharedArrayBuffer",
  "Atomics",
  "DataView",
  "JSON",
  // Control abstraction objects
  "Promise",
  "Generator",
  "GeneratorFunction",
  "AsyncFunction",
  // Reflection
  "Reflect",
  "Proxy",
  // Internationalization
  "Intl",
  // WebAssembly
  "WebAssembly"
], rn = [
  "Error",
  "EvalError",
  "InternalError",
  "RangeError",
  "ReferenceError",
  "SyntaxError",
  "TypeError",
  "URIError"
], sn = [
  "setInterval",
  "setTimeout",
  "clearInterval",
  "clearTimeout",
  "require",
  "exports",
  "eval",
  "isFinite",
  "isNaN",
  "parseFloat",
  "parseInt",
  "decodeURI",
  "decodeURIComponent",
  "encodeURI",
  "encodeURIComponent",
  "escape",
  "unescape"
], ut = [
  "arguments",
  "this",
  "super",
  "console",
  "window",
  "document",
  "localStorage",
  "sessionStorage",
  "module",
  "global"
  // Node.js
], ft = [].concat(
  sn,
  tn,
  rn
);
function bt(s) {
  const f = s.regex, E = (M, { after: j }) => {
    const J = "</" + M[0].slice(1);
    return M.input.indexOf(J, j) !== -1;
  }, b = nn, x = {
    begin: "<>",
    end: "</>"
  }, L = /<[A-Za-z0-9\\._:-]+\s*\/>/, A = {
    begin: /<[A-Za-z0-9\\._:-]+/,
    end: /\/[A-Za-z0-9\\._:-]+>|\/>/,
    /**
     * @param {RegExpMatchArray} match
     * @param {CallbackResponse} response
     */
    isTrulyOpeningTag: (M, j) => {
      const J = M[0].length + M.index, re = M.input[J];
      if (
        // HTML should not include another raw `<` inside a tag
        // nested type?
        // `<Array<Array<number>>`, etc.
        re === "<" || // the , gives away that this is not HTML
        // `<T, A extends keyof T, V>`
        re === ","
      ) {
        j.ignoreMatch();
        return;
      }
      re === ">" && (E(M, { after: J }) || j.ignoreMatch());
      let le;
      const me = M.input.substring(J);
      if (le = me.match(/^\s*=/)) {
        j.ignoreMatch();
        return;
      }
      if ((le = me.match(/^\s+extends\s+/)) && le.index === 0) {
        j.ignoreMatch();
        return;
      }
    }
  }, S = {
    $pattern: nn,
    keyword: dt,
    literal: gt,
    built_in: ft,
    "variable.language": ut
  }, C = "[0-9](_?[0-9])*", v = `\\.(${C})`, U = "0|[1-9](_?[0-9])*|0[0-7]*[89][0-9]*", D = {
    className: "number",
    variants: [
      // DecimalLiteral
      { begin: `(\\b(${U})((${v})|\\.)?|(${v}))[eE][+-]?(${C})\\b` },
      { begin: `\\b(${U})\\b((${v})\\b|\\.)?|(${v})\\b` },
      // DecimalBigIntegerLiteral
      { begin: "\\b(0|[1-9](_?[0-9])*)n\\b" },
      // NonDecimalIntegerLiteral
      { begin: "\\b0[xX][0-9a-fA-F](_?[0-9a-fA-F])*n?\\b" },
      { begin: "\\b0[bB][0-1](_?[0-1])*n?\\b" },
      { begin: "\\b0[oO][0-7](_?[0-7])*n?\\b" },
      // LegacyOctalIntegerLiteral (does not include underscore separators)
      // https://tc39.es/ecma262/#sec-additional-syntax-numeric-literals
      { begin: "\\b0[0-7]+n?\\b" }
    ],
    relevance: 0
  }, B = {
    className: "subst",
    begin: "\\$\\{",
    end: "\\}",
    keywords: S,
    contains: []
    // defined later
  }, z = {
    begin: ".?html`",
    end: "",
    starts: {
      end: "`",
      returnEnd: !1,
      contains: [
        s.BACKSLASH_ESCAPE,
        B
      ],
      subLanguage: "xml"
    }
  }, oe = {
    begin: ".?css`",
    end: "",
    starts: {
      end: "`",
      returnEnd: !1,
      contains: [
        s.BACKSLASH_ESCAPE,
        B
      ],
      subLanguage: "css"
    }
  }, $ = {
    begin: ".?gql`",
    end: "",
    starts: {
      end: "`",
      returnEnd: !1,
      contains: [
        s.BACKSLASH_ESCAPE,
        B
      ],
      subLanguage: "graphql"
    }
  }, ce = {
    className: "string",
    begin: "`",
    end: "`",
    contains: [
      s.BACKSLASH_ESCAPE,
      B
    ]
  }, te = {
    className: "comment",
    variants: [
      s.COMMENT(
        /\/\*\*(?!\/)/,
        "\\*/",
        {
          relevance: 0,
          contains: [
            {
              begin: "(?=@[A-Za-z]+)",
              relevance: 0,
              contains: [
                {
                  className: "doctag",
                  begin: "@[A-Za-z]+"
                },
                {
                  className: "type",
                  begin: "\\{",
                  end: "\\}",
                  excludeEnd: !0,
                  excludeBegin: !0,
                  relevance: 0
                },
                {
                  className: "variable",
                  begin: b + "(?=\\s*(-)|$)",
                  endsParent: !0,
                  relevance: 0
                },
                // eat spaces (not newlines) so we can find
                // types or variables
                {
                  begin: /(?=[^\n])\s/,
                  relevance: 0
                }
              ]
            }
          ]
        }
      ),
      s.C_BLOCK_COMMENT_MODE,
      s.C_LINE_COMMENT_MODE
    ]
  }, ge = [
    s.APOS_STRING_MODE,
    s.QUOTE_STRING_MODE,
    z,
    oe,
    $,
    ce,
    // Skip numbers when they are part of a variable name
    { match: /\$\d+/ },
    D
    // This is intentional:
    // See https://github.com/highlightjs/highlight.js/issues/3288
    // hljs.REGEXP_MODE
  ];
  B.contains = ge.concat({
    // we need to pair up {} inside our subst to prevent
    // it from ending too early by matching another }
    begin: /\{/,
    end: /\}/,
    keywords: S,
    contains: [
      "self"
    ].concat(ge)
  });
  const ue = [].concat(te, B.contains), q = ue.concat([
    // eat recursive parens in sub expressions
    {
      begin: /(\s*)\(/,
      end: /\)/,
      keywords: S,
      contains: ["self"].concat(ue)
    }
  ]), Q = {
    className: "params",
    // convert this to negative lookbehind in v12
    begin: /(\s*)\(/,
    // to match the parms with
    end: /\)/,
    excludeBegin: !0,
    excludeEnd: !0,
    keywords: S,
    contains: q
  }, fe = {
    variants: [
      // class Car extends vehicle
      {
        match: [
          /class/,
          /\s+/,
          b,
          /\s+/,
          /extends/,
          /\s+/,
          f.concat(b, "(", f.concat(/\./, b), ")*")
        ],
        scope: {
          1: "keyword",
          3: "title.class",
          5: "keyword",
          7: "title.class.inherited"
        }
      },
      // class Car
      {
        match: [
          /class/,
          /\s+/,
          b
        ],
        scope: {
          1: "keyword",
          3: "title.class"
        }
      }
    ]
  }, ie = {
    relevance: 0,
    match: f.either(
      // Hard coded exceptions
      /\bJSON/,
      // Float32Array, OutT
      /\b[A-Z][a-z]+([A-Z][a-z]*|\d)*/,
      // CSSFactory, CSSFactoryT
      /\b[A-Z]{2,}([A-Z][a-z]+|\d)+([A-Z][a-z]*)*/,
      // FPs, FPsT
      /\b[A-Z]{2,}[a-z]+([A-Z][a-z]+|\d)*([A-Z][a-z]*)*/
      // P
      // single letters are not highlighted
      // BLAH
      // this will be flagged as a UPPER_CASE_CONSTANT instead
    ),
    className: "title.class",
    keywords: {
      _: [
        // se we still get relevance credit for JS library classes
        ...tn,
        ...rn
      ]
    }
  }, be = {
    label: "use_strict",
    className: "meta",
    relevance: 10,
    begin: /^\s*['"]use (strict|asm)['"]/
  }, pe = {
    variants: [
      {
        match: [
          /function/,
          /\s+/,
          b,
          /(?=\s*\()/
        ]
      },
      // anonymous function
      {
        match: [
          /function/,
          /\s*(?=\()/
        ]
      }
    ],
    className: {
      1: "keyword",
      3: "title.function"
    },
    label: "func.def",
    contains: [Q],
    illegal: /%/
  }, he = {
    relevance: 0,
    match: /\b[A-Z][A-Z_0-9]+\b/,
    className: "variable.constant"
  };
  function Se(M) {
    return f.concat("(?!", M.join("|"), ")");
  }
  const ke = {
    match: f.concat(
      /\b/,
      Se([
        ...sn,
        "super",
        "import"
      ].map((M) => `${M}\\s*\\(`)),
      b,
      f.lookahead(/\s*\(/)
    ),
    className: "title.function",
    relevance: 0
  }, V = {
    begin: f.concat(/\./, f.lookahead(
      f.concat(b, /(?![0-9A-Za-z$_(])/)
    )),
    end: b,
    excludeBegin: !0,
    keywords: "prototype",
    className: "property",
    relevance: 0
  }, Te = {
    match: [
      /get|set/,
      /\s+/,
      b,
      /(?=\()/
    ],
    className: {
      1: "keyword",
      3: "title.function"
    },
    contains: [
      {
        // eat to avoid empty params
        begin: /\(\)/
      },
      Q
    ]
  }, Ee = "(\\([^()]*(\\([^()]*(\\([^()]*\\)[^()]*)*\\)[^()]*)*\\)|" + s.UNDERSCORE_IDENT_RE + ")\\s*=>", Ae = {
    match: [
      /const|var|let/,
      /\s+/,
      b,
      /\s*/,
      /=\s*/,
      /(async\s*)?/,
      // async is optional
      f.lookahead(Ee)
    ],
    keywords: "async",
    className: {
      1: "keyword",
      3: "title.function"
    },
    contains: [
      Q
    ]
  };
  return {
    name: "JavaScript",
    aliases: ["js", "jsx", "mjs", "cjs"],
    keywords: S,
    // this will be extended by TypeScript
    exports: { PARAMS_CONTAINS: q, CLASS_REFERENCE: ie },
    illegal: /#(?![$_A-z])/,
    contains: [
      s.SHEBANG({
        label: "shebang",
        binary: "node",
        relevance: 5
      }),
      be,
      s.APOS_STRING_MODE,
      s.QUOTE_STRING_MODE,
      z,
      oe,
      $,
      ce,
      te,
      // Skip numbers when they are part of a variable name
      { match: /\$\d+/ },
      D,
      ie,
      {
        scope: "attr",
        match: b + f.lookahead(":"),
        relevance: 0
      },
      Ae,
      {
        // "value" container
        begin: "(" + s.RE_STARTERS_RE + "|\\b(case|return|throw)\\b)\\s*",
        keywords: "return throw case",
        relevance: 0,
        contains: [
          te,
          s.REGEXP_MODE,
          {
            className: "function",
            // we have to count the parens to make sure we actually have the
            // correct bounding ( ) before the =>.  There could be any number of
            // sub-expressions inside also surrounded by parens.
            begin: Ee,
            returnBegin: !0,
            end: "\\s*=>",
            contains: [
              {
                className: "params",
                variants: [
                  {
                    begin: s.UNDERSCORE_IDENT_RE,
                    relevance: 0
                  },
                  {
                    className: null,
                    begin: /\(\s*\)/,
                    skip: !0
                  },
                  {
                    begin: /(\s*)\(/,
                    end: /\)/,
                    excludeBegin: !0,
                    excludeEnd: !0,
                    keywords: S,
                    contains: q
                  }
                ]
              }
            ]
          },
          {
            // could be a comma delimited list of params to a function call
            begin: /,/,
            relevance: 0
          },
          {
            match: /\s+/,
            relevance: 0
          },
          {
            // JSX
            variants: [
              { begin: x.begin, end: x.end },
              { match: L },
              {
                begin: A.begin,
                // we carefully check the opening tag to see if it truly
                // is a tag and not a false positive
                "on:begin": A.isTrulyOpeningTag,
                end: A.end
              }
            ],
            subLanguage: "xml",
            contains: [
              {
                begin: A.begin,
                end: A.end,
                skip: !0,
                contains: ["self"]
              }
            ]
          }
        ]
      },
      pe,
      {
        // prevent this from getting swallowed up by function
        // since they appear "function like"
        beginKeywords: "while if switch catch for"
      },
      {
        // we have to count the parens to make sure we actually have the correct
        // bounding ( ).  There could be any number of sub-expressions inside
        // also surrounded by parens.
        begin: "\\b(?!function)" + s.UNDERSCORE_IDENT_RE + "\\([^()]*(\\([^()]*(\\([^()]*\\)[^()]*)*\\)[^()]*)*\\)\\s*\\{",
        // end parens
        returnBegin: !0,
        label: "func.def",
        contains: [
          Q,
          s.inherit(s.TITLE_MODE, { begin: b, className: "title.function" })
        ]
      },
      // catch ... so it won't trigger the property rule below
      {
        match: /\.\.\./,
        relevance: 0
      },
      V,
      // hack: prevents detection of keywords in some circumstances
      // .keyword()
      // $keyword = x
      {
        match: "\\$" + b,
        relevance: 0
      },
      {
        match: [/\bconstructor(?=\s*\()/],
        className: { 1: "title.function" },
        contains: [Q]
      },
      ke,
      he,
      fe,
      Te,
      {
        match: /\$[(.]/
        // relevance booster for a pattern common to JS libs: `$(something)` and `$.something`
      }
    ]
  };
}
function pt(s) {
  const f = s.regex, E = {
    className: "number",
    relevance: 0,
    variants: [
      { begin: /([+-]+)?[\d]+_[\d_]+/ },
      { begin: s.NUMBER_RE }
    ]
  }, b = s.COMMENT();
  b.variants = [
    {
      begin: /;/,
      end: /$/
    },
    {
      begin: /#/,
      end: /$/
    }
  ];
  const x = {
    className: "variable",
    variants: [
      { begin: /\$[\w\d"][\w\d_]*/ },
      { begin: /\$\{(.*?)\}/ }
    ]
  }, L = {
    className: "literal",
    begin: /\bon|off|true|false|yes|no\b/
  }, A = {
    className: "string",
    contains: [s.BACKSLASH_ESCAPE],
    variants: [
      {
        begin: "'''",
        end: "'''",
        relevance: 10
      },
      {
        begin: '"""',
        end: '"""',
        relevance: 10
      },
      {
        begin: '"',
        end: '"'
      },
      {
        begin: "'",
        end: "'"
      }
    ]
  }, S = {
    begin: /\[/,
    end: /\]/,
    contains: [
      b,
      L,
      x,
      A,
      E,
      "self"
    ],
    relevance: 0
  }, C = /[A-Za-z0-9_-]+/, v = /"(\\"|[^"])*"/, U = /'[^']*'/, D = f.either(
    C,
    v,
    U
  ), B = f.concat(
    D,
    "(\\s*\\.\\s*",
    D,
    ")*",
    f.lookahead(/\s*=\s*[^#\s]/)
  );
  return {
    name: "TOML, also INI",
    aliases: ["toml"],
    case_insensitive: !0,
    illegal: /\S/,
    contains: [
      b,
      {
        className: "section",
        begin: /\[+/,
        end: /\]+/
      },
      {
        begin: B,
        className: "attr",
        starts: {
          end: /$/,
          contains: [
            b,
            S,
            L,
            x,
            A,
            E
          ]
        }
      }
    ]
  };
}
function ht(s) {
  const f = s.regex;
  return {
    name: "Diff",
    aliases: ["patch"],
    contains: [
      {
        className: "meta",
        relevance: 10,
        match: f.either(
          /^@@ +-\d+,\d+ +\+\d+,\d+ +@@/,
          /^\*\*\* +\d+,\d+ +\*\*\*\*$/,
          /^--- +\d+,\d+ +----$/
        )
      },
      {
        className: "comment",
        variants: [
          {
            begin: f.either(
              /Index: /,
              /^index/,
              /={3,}/,
              /^-{3}/,
              /^\*{3} /,
              /^\+{3}/,
              /^diff --git/
            ),
            end: /$/
          },
          { match: /^\*{15}$/ }
        ]
      },
      {
        className: "addition",
        begin: /^\+/,
        end: /$/
      },
      {
        className: "deletion",
        begin: /^-/,
        end: /$/
      },
      {
        className: "addition",
        begin: /^!/,
        end: /$/
      }
    ]
  };
}
function Et(s) {
  return {
    name: "Plain text",
    aliases: [
      "text",
      "txt"
    ],
    disableAutodetect: !0
  };
}
F.registerLanguage("lua", Vn);
F.registerLanguage("xml", Jn);
F.registerLanguage("markdown", et);
F.registerLanguage("css", lt);
F.registerLanguage("javascript", bt);
F.registerLanguage("ini", pt);
F.registerLanguage("diff", ht);
F.registerLanguage("plaintext", Et);
const mt = {
  lua: "lua",
  xml: "xml",
  html: "xml",
  htm: "xml",
  toc: "ini",
  md: "markdown",
  markdown: "markdown",
  css: "css",
  js: "javascript",
  mjs: "javascript",
  json: "javascript",
  diff: "diff",
  patch: "diff",
  txt: "plaintext",
  log: "plaintext"
};
function Nt(s, f) {
  const E = (f.split(".").pop() || "").toLowerCase(), b = mt[E];
  if (b)
    try {
      return F.highlight(s, { language: b }).value;
    } catch {
    }
  try {
    const x = F.highlightAuto(s);
    if (x.relevance > 2) return x.value;
  } catch {
  }
  return null;
}
export {
  Nt as highlightCode
};
