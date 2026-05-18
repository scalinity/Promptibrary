// Typed variable system per spec §4 "Variable" and §5 syntax grammar.
//
// The discriminated union is keyed on `type`. Each variant carries the
// type-specific constraints alongside the shared `VariableBase` fields.

import type { AbsolutePath } from "./ids";

export type VariableType =
  | "file"
  | "folder"
  | "text"
  | "multiline"
  | "select"
  | "bool"
  | "number";

export type VariableSource = "parsed" | "frontmatter";

export interface VariableBase<TType extends VariableType, TValue> {
  key: string;
  type: TType;
  label: string;
  description: string | null;
  required: boolean;
  defaultValue: TValue | null;
  order: number;
  source: VariableSource;
}

export interface FileVariable extends VariableBase<"file", AbsolutePath> {
  mustExist: boolean;
  allowedExtensions: string[];
  allowMultiple: false;
}

export interface FolderVariable extends VariableBase<"folder", AbsolutePath> {
  mustExist: boolean;
  mustBeWritable: boolean;
}

export interface TextVariable extends VariableBase<"text", string> {
  minLength: number | null;
  maxLength: number | null;
  pattern: string | null;
  trim: boolean;
}

export interface MultilineVariable extends VariableBase<"multiline", string> {
  minLength: number | null;
  maxLength: number | null;
  trimTrailingWhitespace: boolean;
}

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectVariable extends VariableBase<"select", string> {
  options: SelectOption[];
  allowCustom: false;
}

export interface BoolVariable extends VariableBase<"bool", boolean> {
  renderTrue: string;
  renderFalse: string;
}

export interface NumberVariable extends VariableBase<"number", number> {
  min: number | null;
  max: number | null;
  step: number | null;
  integer: boolean;
}

export type Variable =
  | FileVariable
  | FolderVariable
  | TextVariable
  | MultilineVariable
  | SelectVariable
  | BoolVariable
  | NumberVariable;
