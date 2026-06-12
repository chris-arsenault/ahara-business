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
  noNonVitestTesting,
  noJsFileExtension,
} from "@ahara/standards/eslint-rules";

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
      "max-lines": [
        "error",
        { max: 400, skipBlankLines: true, skipComments: true },
      ],
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
          "no-non-vitest-testing": noNonVitestTesting,
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
      "local/no-non-vitest-testing": "error",
      "local/no-js-file-extension": "error",
    },
  },

  sonarjs.configs.recommended,

  prettier,
);
