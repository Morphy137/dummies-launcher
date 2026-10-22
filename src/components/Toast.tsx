import { Dismiss16Regular } from "@fluentui/react-icons";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

interface ToastProps {
  message: string;
  actionLabel?: string;
  duration?: number;
  onAction?: () => void;
  onDismiss: () => void;
}

export default function Toast({
  message,
  actionLabel,
  duration = 3200,
  onAction,
  onDismiss,
}: ToastProps) {
  const { t } = useTranslation();
  const dismissRef = useRef(onDismiss);

  useEffect(() => {
    dismissRef.current = onDismiss;
  }, [onDismiss]);

  useEffect(() => {
    const timer = window.setTimeout(() => dismissRef.current(), duration);
    return () => window.clearTimeout(timer);
  }, [duration]);

  return (
    <div className="toast" role="status" aria-live="polite">
      <span className="toast__message">{message}</span>
      {actionLabel && onAction ? (
        <button
          type="button"
          className="toast__action"
          onClick={() => {
            onAction();
            dismissRef.current();
          }}
        >
          {actionLabel}
        </button>
      ) : null}
      <button
        type="button"
        className="toast__close"
        onClick={() => dismissRef.current()}
        aria-label={t("common.dismiss")}
      >
        <Dismiss16Regular />
      </button>
    </div>
  );
}
