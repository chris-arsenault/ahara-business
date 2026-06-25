type AppMarkProps = Partial<{
  className: string;
  size: number;
}>;

export function AppMark({ className, size = 36 }: AppMarkProps) {
  const classNames = ["app-mark", className].filter(Boolean).join(" ");
  return (
    <svg
      aria-hidden="true"
      className={classNames}
      focusable="false"
      height={size}
      viewBox="0 0 64 64"
      width={size}
    >
      <rect className="app-mark-paper" height="64" rx="14" width="64" />
      <path
        className="app-mark-spine"
        d="M12 17h40a5 5 0 0 1 5 5v27a5 5 0 0 1-5 5H12a5 5 0 0 1-5-5V22a5 5 0 0 1 5-5Z"
      />
      <path
        className="app-mark-envelope"
        d="M12 23.6v-1.8h40v1.8L32 38.1 12 23.6Zm0 5.7 14.2 10.2L12 48.8V29.3Zm40 0v19.5l-14.2-9.3L52 29.3ZM32 43.4l3.7-2.6L50 50H14l14.3-9.2 3.7 2.6Z"
      />
      <circle className="app-mark-seal" cx="46" cy="46" r="10" />
    </svg>
  );
}
