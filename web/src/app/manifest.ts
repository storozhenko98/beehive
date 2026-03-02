import type { MetadataRoute } from "next";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "Beehive",
    short_name: "Beehive",
    description:
      "Orchestrate coding agents across isolated git workspaces.",
    start_url: "/",
    display: "browser",
    background_color: "#11111b",
    theme_color: "#89b4fa",
    icons: [
      {
        src: "/icon-192.png",
        sizes: "192x192",
        type: "image/png",
      },
      {
        src: "/icon-512.png",
        sizes: "512x512",
        type: "image/png",
      },
    ],
  };
}
