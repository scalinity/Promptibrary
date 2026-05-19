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
  // CLAUDE.md "React Architecture & Side-Effects Policy" strict rule:
  // never use `useEffect` directly. Mechanically enforce by forbidding
  // the named import from React. If a genuine escape hatch is unavoidable
  // after exhausting the five replacement patterns (derived state, data-
  // fetching hooks, event handlers, `key` remount, useMountEffect),
  // opt out with `// eslint-disable-next-line no-restricted-imports`
  // plus a rationale comment — that's the SCA-722 escape valve.
  paths: [
    {
      name: "react",
      importNames: ["useEffect"],
      message:
        "Direct useEffect is banned by CLAUDE.md. Prefer derived state, a data-fetching hook (TanStack Query/SWR), an event handler, the `key` remount, or useMountEffect. Opt out with `// eslint-disable-next-line no-restricted-imports` + a rationale.",
    },
  ],
};

export default tseslint.config(
  {
    ignores: [
      "dist",
      "node_modules",
      "src-tauri/target",
      "src-tauri/gen",
      "playwright-report",
      "test-results",
      // SCA-624: parallel-agent worktrees under .claude/ contain
      // generated Tauri assets (tauri-codegen-assets/*.js) that aren't
      // valid JS and aren't part of our checked-in source. Lint must
      // not scan them.
      ".claude/**",
    ],
  },
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
    // Wrapper layer is allowed to import shadcn primitives directly, but
    // it STILL gets the useEffect ban — we only re-enable the shadcn path
    // here, keeping the `paths` restriction (no direct useEffect) intact.
    files: ["src/shared/ui/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "react",
              importNames: ["useEffect"],
              message:
                "Direct useEffect is banned by CLAUDE.md. Prefer derived state, a data-fetching hook, an event handler, the `key` remount, or useMountEffect. Opt out with `// eslint-disable-next-line no-restricted-imports` + a rationale.",
            },
          ],
          // No shadcn `patterns` entry here — wrappers may import shadcn.
        },
      ],
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
