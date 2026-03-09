import { ImageResponse } from "next/og";

export const runtime = "edge";
export const alt = "Beehive — Orchestrate Coding Agents";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default async function OGImage() {
  return new ImageResponse(
    (
      <div
        style={{
          background: "#11111b",
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          fontFamily: "sans-serif",
        }}
      >
        {/* Subtle gradient glow */}
        <div
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            background:
              "radial-gradient(ellipse 60% 50% at 50% 50%, rgba(137,180,250,0.08) 0%, transparent 70%)",
          }}
        />
        {/* Logo placeholder — hexagon shape */}
        <div
          style={{
            width: 100,
            height: 100,
            borderRadius: 24,
            background: "linear-gradient(135deg, #89b4fa, #74c7ec)",
            marginBottom: 32,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 48,
            color: "#11111b",
            fontWeight: 800,
          }}
        >
          B
        </div>
        <div
          style={{
            fontSize: 64,
            fontWeight: 800,
            letterSpacing: -2,
            color: "#cdd6f4",
            marginBottom: 16,
          }}
        >
          Beehive
        </div>
        <div
          style={{
            fontSize: 24,
            color: "#a6adc8",
            maxWidth: 600,
            textAlign: "center",
            lineHeight: 1.4,
          }}
        >
          Orchestrate coding agents across isolated git workspaces
        </div>
        {/* Footer pills */}
        <div
          style={{
            display: "flex",
            gap: 16,
            marginTop: 40,
          }}
        >
          {["macOS + Linux", "GUI + TUI", "Open Source"].map((label) => (
            <div
              key={label}
              style={{
                background: "#313244",
                color: "#a6adc8",
                padding: "8px 20px",
                borderRadius: 8,
                fontSize: 16,
                fontWeight: 500,
              }}
            >
              {label}
            </div>
          ))}
        </div>
      </div>
    ),
    { ...size }
  );
}
