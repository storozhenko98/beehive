import { createConnection } from "node:net";

type ExtensionEvent = { reason?: string };
type ExtensionHandler = (event: ExtensionEvent) => void | Promise<void>;
interface ExtensionAPI {
  on(event: string, handler: ExtensionHandler): void;
}

const port = Number(process.env.BEEHIVE_PI_STATUS_PORT);

function notify(status: "clear" | "idle" | "error" | "stopped", eventType: string): void {
  if (!Number.isInteger(port) || port < 1 || port > 65535) return;

  const socket = createConnection({ host: "127.0.0.1", port }, () => {
    socket.end(`${JSON.stringify({ status, eventType })}\n`);
  });
  socket.on("error", () => {});
  socket.unref();
}

export default function beehiveStatus(pi: ExtensionAPI): void {
  let settledSupported = false;
  let fallbackTimer: ReturnType<typeof setTimeout> | undefined;

  pi.on("session_start", async () => notify("idle", "session_start"));
  pi.on("agent_start", async () => {
    clearTimeout(fallbackTimer);
    notify("clear", "agent_start");
  });
  pi.on("agent_end", async () => {
    if (!settledSupported) {
      fallbackTimer = setTimeout(() => notify("idle", "agent_end"), 500);
    }
  });
  pi.on("agent_settled", async () => {
    settledSupported = true;
    clearTimeout(fallbackTimer);
    notify("idle", "agent_settled");
  });
  pi.on("session_shutdown", async () => notify("clear", "session_shutdown"));
}
