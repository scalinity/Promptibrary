// Wrapper re-export for the shadcn Button primitive.
//
// Per the ESLint `no-restricted-imports` boundary (see eslint.config.js), the
// rest of the app must import Button through this file, not from
// `@/shared/ui/shadcn/button` directly. This wrapper layer is where
// Promptibrary tokens are applied (over time) without forking the shadcn
// primitive itself.

export { Button, buttonVariants, type ButtonProps } from "./shadcn/button";
