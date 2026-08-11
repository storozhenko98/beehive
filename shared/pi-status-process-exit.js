const { createConnection } = require("node:net");

const port = Number(process.env.BEEHIVE_PI_STATUS_PORT);
if (Number.isInteger(port) && port >= 1 && port <= 65535) {
  const socket = createConnection({ host: "127.0.0.1", port }, () => {
    socket.end(`${JSON.stringify({ status: "stopped", eventType: "process_exit" })}\n`);
  });
  socket.on("error", () => {});
}
