// Headless dropdown / combobox (SCA-904).
//
// Replaces native `<select>` in settings panels so the chrome matches
// the carbon palette. The trigger is borderless ("just text + ▾"), the
// popover floats below with --bg-raised + hairline + soft shadow.
//
// Keyboard: ↑/↓ navigate, Enter selects, Esc / Tab / outside-click closes.
// Token-driven throughout — no colour literals.

// eslint-disable-next-line no-restricted-imports -- SCA-904: outside-click subscription has no declarative equivalent
import { useEffect, useId, useRef, useState } from "react";

export interface DropdownOption<T extends string> {
  value: T;
  label: string;
}

interface DropdownProps<T extends string> {
  value: T | undefined;
  options: readonly DropdownOption<T>[];
  onChange: (v: T) => void;
  disabled?: boolean;
  ariaLabel?: string;
}

export function Dropdown<T extends string>({
  value,
  options,
  onChange,
  disabled = false,
  ariaLabel,
}: DropdownProps<T>): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const listboxId = useId();

  const selectedIndex = options.findIndex((o) => o.value === value);
  const currentLabel = selectedIndex >= 0 ? options[selectedIndex]!.label : "—";

  // Sync the active highlight to the selected option whenever the
  // popover opens — this is an "external sync with browser focus
  // semantics" usage of useEffect (DOM state ⇆ React state), the
  // narrow case CLAUDE.md leaves open.
  useEffect(() => {
    if (open) {
      setActiveIndex(selectedIndex >= 0 ? selectedIndex : 0);
    }
  }, [open, selectedIndex]);

  // Outside-click + Escape close. Subscribed only while open so the
  // listeners aren't a permanent fixture on every dropdown mount.
  useEffect(() => {
    if (!open) return;
    const onPointer = (e: PointerEvent) => {
      const target = e.target as Node | null;
      if (
        target != null &&
        !triggerRef.current?.contains(target) &&
        !panelRef.current?.contains(target)
      ) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener("pointerdown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const commit = (next: T): void => {
    onChange(next);
    setOpen(false);
    triggerRef.current?.focus();
  };

  const onTriggerKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>): void => {
    if (disabled) return;
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      setOpen(true);
    }
  };

  const onListKeyDown = (e: React.KeyboardEvent<HTMLDivElement>): void => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => (i + 1) % options.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => (i - 1 + options.length) % options.length);
    } else if (e.key === "Home") {
      e.preventDefault();
      setActiveIndex(0);
    } else if (e.key === "End") {
      e.preventDefault();
      setActiveIndex(options.length - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const opt = options[activeIndex];
      if (opt) commit(opt.value);
    } else if (e.key === "Tab") {
      // Allow Tab to close + move focus naturally — no preventDefault.
      setOpen(false);
    }
  };

  return (
    <div style={wrapperStyle}>
      <button
        ref={triggerRef}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listboxId : undefined}
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={() => {
          if (!disabled) setOpen((v) => !v);
        }}
        onKeyDown={onTriggerKeyDown}
        style={triggerStyle(disabled)}
      >
        <span>{currentLabel}</span>
        <span aria-hidden="true" style={chevronStyle(open)}>
          ▾
        </span>
      </button>
      {open && (
        <div
          ref={panelRef}
          id={listboxId}
          role="listbox"
          tabIndex={-1}
          onKeyDown={onListKeyDown}
          style={panelStyle}
        >
          {/* Focus the panel on first render so arrow keys work. */}
          <PanelFocusOnMount panelRef={panelRef} />
          {options.map((opt, i) => {
            const isSelected = opt.value === value;
            const isActive = i === activeIndex;
            return (
              <button
                key={opt.value}
                type="button"
                role="option"
                aria-selected={isSelected}
                onMouseEnter={() => setActiveIndex(i)}
                onClick={() => commit(opt.value)}
                style={optionStyle(isActive, isSelected)}
              >
                <span style={optionCheckStyle(isSelected)}>✓</span>
                <span>{opt.label}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

// Focus the panel on mount so arrow keys are caught by onKeyDown.
// Implemented as a tiny effect-only component because the panel ref is
// owned by Dropdown — keeping that ref stable means avoiding a function
// ref on the panel div itself.
function PanelFocusOnMount({
  panelRef,
}: {
  panelRef: React.RefObject<HTMLDivElement | null>;
}): null {
  useEffect(() => {
    panelRef.current?.focus();
  }, [panelRef]);
  return null;
}

const wrapperStyle: React.CSSProperties = {
  position: "relative",
  display: "inline-block",
};

function triggerStyle(disabled: boolean): React.CSSProperties {
  return {
    display: "inline-flex",
    alignItems: "center",
    gap: 6,
    background: "transparent",
    border: "none",
    padding: 0,
    fontFamily: "var(--font-mono)",
    fontSize: "12.5px",
    color: disabled ? "var(--ink-tertiary)" : "var(--ink-primary)",
    cursor: disabled ? "not-allowed" : "pointer",
  };
}

function chevronStyle(open: boolean): React.CSSProperties {
  return {
    fontSize: "10px",
    color: "var(--ink-tertiary)",
    transition: "transform var(--motion-snap)",
    transform: open ? "rotate(180deg)" : "rotate(0deg)",
  };
}

const panelStyle: React.CSSProperties = {
  position: "absolute",
  top: "calc(100% + 6px)",
  left: 0,
  minWidth: "180px",
  background: "var(--bg-raised)",
  border: "1px solid var(--border-mid)",
  borderRadius: "var(--r-md)",
  boxShadow: "0 8px 24px -8px var(--shadow-modal)",
  padding: "4px",
  display: "flex",
  flexDirection: "column",
  gap: 2,
  zIndex: 50,
  outline: "none",
};

function optionStyle(active: boolean, selected: boolean): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 8,
    padding: "6px 10px",
    background: active ? "var(--accent-tint)" : "transparent",
    border: "none",
    borderRadius: "var(--r-sm)",
    fontFamily: "var(--font-mono)",
    fontSize: "12.5px",
    color: selected ? "var(--ink-primary)" : "var(--ink-secondary)",
    cursor: "pointer",
    textAlign: "left",
    whiteSpace: "nowrap",
  };
}

function optionCheckStyle(selected: boolean): React.CSSProperties {
  return {
    fontSize: "10px",
    color: selected ? "var(--accent)" : "transparent",
    width: "12px",
    flexShrink: 0,
  };
}
