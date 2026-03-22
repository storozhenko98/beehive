interface ToastItem {
  id: string;
  message: string;
  type: "error" | "success";
}

interface Props {
  toasts: ToastItem[];
  onDismiss: (id: string) => void;
}

export function Toast({ toasts, onDismiss }: Props) {
  if (toasts.length === 0) return null;

  return (
    <div className="toast-container">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast toast-${toast.type}`}>
          <span className="toast-message">{toast.message}</span>
          <button
            className="toast-dismiss"
            onClick={() => onDismiss(toast.id)}
            aria-label="Dismiss"
          >
            x
          </button>
        </div>
      ))}
    </div>
  );
}
