export type ArtworkTypeIconVariant =
  "horizontal" | "vertical" | "square" | "hero" | "logo";

export function ArtworkTypeIcon({
  variant,
  className,
}: {
  variant: ArtworkTypeIconVariant;
  className?: string;
}) {
  const classNames = ["artwork-type-icon", className].filter(Boolean).join(" ");

  return (
    <svg
      aria-hidden="true"
      className={classNames}
      viewBox="0 0 28 28"
      fill="none"
      focusable="false"
    >
      {variant === "horizontal" && (
        <>
          <rect x="3" y="8" width="22" height="12" rx="2.5" />
          <path d="m5.5 17 4.2-4 3.1 2.5 3.2-3 6.5 5.2" />
          <circle cx="19.5" cy="11.5" r="1.2" />
        </>
      )}
      {variant === "vertical" && (
        <>
          <rect x="8.5" y="3" width="11" height="22" rx="2.5" />
          <path d="m10.5 20 2.8-3.3 2.1 1.7 2.8-3.1" />
          <circle cx="16.5" cy="8" r="1.1" />
        </>
      )}
      {variant === "square" && (
        <>
          <rect x="5" y="5" width="18" height="18" rx="2.5" />
          <path d="m7.5 19 3.6-4 3.2 2.5 3.1-3 3.1 3.5" />
          <circle cx="18.5" cy="9.5" r="1.2" />
        </>
      )}
      {variant === "hero" && (
        <>
          <rect x="2.5" y="9" width="23" height="10" rx="2.5" />
          <path d="M4.5 16.5c2.4-3.7 4.8-4.2 7.2-1.5 2.1 2.4 3.8 2.1 5.5-.5 1.3-2 3.1-2.1 6.3.5" />
          <path d="M7 12.5h.1M21 12.5h.1" strokeWidth="2.4" />
        </>
      )}
      {variant === "logo" && (
        <>
          <path d="M7 5.5c2.6-.9 4.1.8 5.8-.1 2.1-1.1 3.1.7 5.1.4 2.9-.4 4.6 1.3 3.9 3.8-.5 1.8.9 3.2-.3 5.1-1.2 2-3.1 1.5-4.8 2.4-2.1 1.1-3.6-.4-5.5.4-2.7 1.1-4.9-.4-4.2-2.9.5-1.9-1.2-3.5 0-5.2Z" />
          <path
            d="M9.5 10.5h.1M14 8.8h.1M18.5 12h.1M12 14.5h.1"
            strokeWidth="2.4"
          />
        </>
      )}
    </svg>
  );
}
