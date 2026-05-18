// `cn` re-export. shadcn defaults its `cn` import path to `@/shared/lib/utils`
// (see components.json), but the spec §3 file tree also lists `cn.ts`. This
// keeps `import { cn } from "@/shared/lib/cn"` working without duplicating logic.

export { cn } from "./utils";
