import type { languages } from "monaco-editor";

export const er2LanguageId = "er2";

// ArgScript parser: StripCommentsAndWhiteSpace, SubstituteVariables.
// ER2 commands and references: HeroesAndVillains AEC 573AD901 / A3369E07.
export const er2Tokens: languages.IMonarchLanguage = {
  ignoreCase: true,
  includeLF: true,
  defaultToken: "",
  directives: [
    "version", "include", "pushdir", "popdir", "define", "redefine",
    "enddef", "create", "if", "elseif", "else", "endif", "set",
    "namespace", "purge", "enum", "flag", "mode", "output", "error",
  ],
  commands: [
    "unitrule", "globalrule", "end", "local", "nearby", "connected",
    "agent", "map", "global", "rate", "timetrigger", "priority", "applycount",
    "maxapplycount", "onsuccess", "onfail", "chain", "flags", "globalflags",
    "options", "testagent", "successevent",
  ],
  operands: ["in", "out", "is", "atleast", "atmost", "true", "false"],
  substitutions: /(?:[$&]\{[\w:]+\}|\$[\w:]+)/,
  tokenizer: {
    root: [
      [/^([ \t]*)([a-z_]\w*)/, [
        "",
        { cases: { "@directives": "keyword.control", "@commands": "keyword", "@default": "identifier" } },
      ]],
      [/#</, "comment", "@blockComment"],
      [/#>/, "delimiter"],
      [/#.*/, "comment"],
      [/"/, "string", "@doubleString"],
      [/'/, "string", "@singleString"],
      [/@substitutions/, "variable"],
      [/!?[a-z_]\w*@[a-z_]\w*/, "variable"],
      [/-[a-z][\w-]*/, "attribute.name"],
      [/[+-]?(?:0x[\da-f]+|(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?)(?![\w.])/, "number"],
      [/[a-z_]\w*/, { cases: { "@operands": "keyword.operator", "@default": "identifier" } }],
      [/[{}()[\],]/, "delimiter"],
      [/[=!<>]=?|&&|\|\||[+*/-]/, "operator"],
    ],
    blockComment: [
      [/#</, "comment", "@push"],
      [/#>/, "comment", "@pop"],
      [/[^#]+|#/, "comment"],
    ],
    doubleString: [
      [/@substitutions/, "variable"],
      [/"/, "string", "@pop"],
      [/\n/, "", "@pop"],
      [/[^"\n$&]+|[$&]/, "string"],
    ],
    singleString: [
      [/@substitutions/, "variable"],
      [/'/, "string", "@pop"],
      [/\n/, "", "@pop"],
      [/[^'\n$&]+|[$&]/, "string"],
    ],
  },
};

export function registerEr2(languagesApi: typeof languages) {
  if (languagesApi.getLanguages().some(({ id }) => id === er2LanguageId)) return;
  languagesApi.register({ id: er2LanguageId, extensions: [".er2", ".aec"] });
  languagesApi.setMonarchTokensProvider(er2LanguageId, er2Tokens);
}
