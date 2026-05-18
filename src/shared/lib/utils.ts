// shadcn convention: `cn` lives at `@/shared/lib/utils` and merges class names
// using clsx for conditional logic + tailwind-merge for last-wins precedence.

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
