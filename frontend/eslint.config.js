import js from "@eslint/js";
import globals from "globals";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import reactPerf from "eslint-plugin-react-perf";
import jsxA11y from "eslint-plugin-jsx-a11y";
import sonarjs from "eslint-plugin-sonarjs";
import prettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";
import {
  maxJsxProps,
  noInlineStyles,
  noDirectFetch,
  noDirectStoreImport,
  noEscapeHatches,
  noManualAsyncState,
  noManualExpandState,
  noManualViewHeader,
  noNonVitestTesting,
  noRawUndefinedUnion,
  noJsFileExtension,
} from "@ahara/standards/eslint-rules";

const withEslint9ContextCompat = (rule) => ({
  ...rule,
  create(context) {
    const sourceCode = context.sourceCode ?? context.getSourceCode();
    const compatContext = Object.create(context);

    compatContext.getCommentsBefore = (node) =>
      sourceCode.getCommentsBefore(node);

    return rule.create(compatContext);
  },
});

export default tseslint.config(
  {
    ignores: ["dist/", "node_modules/", "coverage/"],
  },

  {
    ...js.configs.recommended,
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
    },
    rules: {
      complexity: ["error", 10],
      "max-lines-per-function": [
        "error",
        { max: 75, skipBlankLines: true, skipComments: true },
      ],
      "max-depth": ["warn", 4],
    },
  },

  ...tseslint.configs.recommended,

  {
    files: ["src/**/*.{ts,tsx}"],
    plugins: {
      react,
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
      "react-perf": reactPerf,
      "jsx-a11y": jsxA11y,
      local: {
        rules: {
          "max-jsx-props": maxJsxProps,
          "no-inline-styles": noInlineStyles,
          "no-direct-fetch": noDirectFetch,
          "no-direct-store-import": noDirectStoreImport,
          "no-escape-hatches": withEslint9ContextCompat(noEscapeHatches),
          "no-manual-async-state": noManualAsyncState,
          "no-manual-expand-state": noManualExpandState,
          "no-manual-view-header": noManualViewHeader,
          "no-non-vitest-testing": noNonVitestTesting,
          "no-raw-undefined-union": noRawUndefinedUnion,
          "no-js-file-extension": noJsFileExtension,
        },
      },
    },
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.es2025,
      },
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
    },
    settings: {
      react: { version: "detect" },
    },
    rules: {
      ...react.configs.recommended.rules,
      ...reactHooks.configs.recommended.rules,
      ...jsxA11y.configs.recommended.rules,
      complexity: ["error", 10],
      "max-lines": [
        "error",
        { max: 400, skipBlankLines: true, skipComments: true },
      ],
      "max-lines-per-function": [
        "error",
        { max: 75, skipBlankLines: true, skipComments: true },
      ],
      "max-depth": ["warn", 4],
      "react/react-in-jsx-scope": "off",
      "react/prop-types": "off",
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_" },
      ],
      "no-unused-vars": "off",
      "react-perf/jsx-no-new-object-as-prop": [
        "warn",
        { nativeAllowList: "all" },
      ],
      "react-perf/jsx-no-new-array-as-prop": [
        "warn",
        { nativeAllowList: "all" },
      ],
      "react-perf/jsx-no-new-function-as-prop": [
        "warn",
        { nativeAllowList: "all" },
      ],
      "local/max-jsx-props": ["warn", { max: 12 }],
      "local/no-inline-styles": "error",
      "local/no-direct-fetch": "error",
      "local/no-direct-store-import": "warn",
      "local/no-escape-hatches": "error",
      "local/no-manual-async-state": "warn",
      "local/no-manual-expand-state": "warn",
      "local/no-manual-view-header": "warn",
      "local/no-non-vitest-testing": "error",
      "local/no-raw-undefined-union": "warn",
      "local/no-js-file-extension": "error",
    },
  },

  {
    files: ["src/mailbox.tsx", "src/mailbox.test.tsx", "src/routingAdmin.tsx"],
    rules: {
      "max-lines": "off",
    },
  },

  sonarjs.configs.recommended,

  prettier,
);
