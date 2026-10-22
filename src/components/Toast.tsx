import { Dismiss16Regular } from "@fluentui/react-icons";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

interface ToastProps {
  message: string;
  onDismiss: () => void;
}

export default function Toast({ message, onDismiss }: ToastProps) {
  const { t } = useTranslation();
  const dismissRef = useRef(onDismiss);

  useEffect(() => {
    dismissRef.current = onDismiss;
  }, [onDismiss]);

  useEffect(() => {
    const timer = window.setTimeout(() => dismissRef.current(), 3200);
    return () => window.clearTimeout(timer);
  }, []);

  return (
    <div className="toast" role="status" aria-live="polite">
      <span className="toast__message">{message}</span>
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
