import { forwardRef, type CSSProperties, type ReactNode } from "react";

import { useFocusable, type UseFocusableOptions } from "./useFocusable";

export interface FocusableProps extends UseFocusableOptions {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  ariaLabel?: string;
  ariaCurrent?: "page" | "step" | "location" | "date" | "time" | true | false;
  ariaSelected?: boolean;
}

export const Focusable = forwardRef<HTMLButtonElement, FocusableProps>(
  function Focusable(
    {
      children,
      className,
      style,
      ariaLabel,
      ariaCurrent,
      ariaSelected,
      ...focusOptions
    },
    forwardedRef,
  ) {
    const focusable = useFocusable<HTMLButtonElement>(focusOptions);
    const setRef = (element: HTMLButtonElement | null) => {
      focusable.ref.current = element;
      if (typeof forwardedRef === "function") {
        forwardedRef(element);
      } else if (forwardedRef) {
        forwardedRef.current = element;
      }
    };

    return (
      <button
        ref={setRef}
        className={className}
        style={style}
        type="button"
        disabled={focusOptions.disabled}
        tabIndex={focusable.tabIndex}
        aria-label={ariaLabel}
        aria-current={ariaCurrent}
        aria-selected={ariaSelected}
        aria-disabled={focusOptions.disabled || undefined}
        data-focusable="true"
        data-focus-id={focusOptions.focusId}
        data-active={focusable.isActive ? "true" : "false"}
        onMouseEnter={focusable.onMouseEnter}
        onClick={focusable.onClick}
      >
        {children}
      </button>
    );
  },
);
