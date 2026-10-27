// Tauri subscriptions resolve asynchronously, possibly after React has unmounted.
export function disposeSubscription(pending: Promise<() => void>): () => void {
  return () => {
    void pending.then((unlisten) => unlisten()).catch(() => {});
  };
}
