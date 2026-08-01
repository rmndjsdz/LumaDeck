import { forwardRef, type CSSProperties, type ReactNode } from "react";

import { useFocusable, type UseFocusableOptions } from "./useFocusable";

export interface FocusableProps extends UseFocusableOptions {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  ariaLabel?: string;
}

export const Focusable = forwardRef<HTMLDivElement, FocusableProps>(
  function Focusable(
    { children, className, style, ariaLabel, ...focusOptions },
    forwardedRef,
  ) {
    const focusable = useFocusable<HTMLDivElement>(focusOptions);
    const setRef = (element: HTMLDivElement | null) => {
      focusable.ref.current = element;
      if (typeof forwardedRef === "function") {
        forwardedRef(element);
      } else if (forwardedRef) {
        forwardedRef.current = element;
      }
    };

    return (
      <div
        ref={setRef}
        className={className}
        style={style}
        role="button"
        tabIndex={focusable.tabIndex}
        aria-label={ariaLabel}
        aria-disabled={focusOptions.disabled || undefined}
        data-focusable="true"
        data-focus-id={focusOptions.focusId}
        data-active={focusable.isActive ? "true" : "false"}
        onMouseEnter={focusable.onMouseEnter}
        onClick={focusable.onClick}
      >
        {children}
      </div>
    );
  },
);
