// @ts-check
import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";

const SHADCN_IMPORT_BOUNDARY = {
  // Architecture invariant per spec §2 / CLAUDE.md:
  // only `@/shared/ui/*` files may import shadcn primitives directly.
  // Everything else must use the wrapper layer at `@/shared/ui/<name>`.
  patterns: [
    {
      group: ["@/shared/ui/shadcn", "@/shared/ui/shadcn/*"],
      message:
        "Import shadcn primitives only via the @/shared/ui/* wrapper layer, not directly.",
    },
  ],
};

export default tseslint.config(
  { ignores: ["dist", "node_modules", "src-tauri/target", "src-tauri/gen", "playwright-report", "test-results"] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2023,
      globals: globals.browser,
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
      "no-restricted-imports": ["error", SHADCN_IMPORT_BOUNDARY],
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },
  {
    // Wrapper layer is allowed to import shadcn primitives directly.
    files: ["src/shared/ui/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": "off",
    },
  },
  {
    // Config / test runtimes use node globals.
    files: [
      "vite.config.ts",
      "vitest.config.ts",
      "playwright.config.ts",
      "eslint.config.js",
      "src/**/*.test.{ts,tsx}",
      "src/__tests__/**/*.{ts,tsx}",
      "e2e/**/*.{ts,tsx}",
    ],
    languageOptions: {
      globals: { ...globals.node, ...globals.browser },
    },
  },
);
