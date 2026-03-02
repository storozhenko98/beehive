import type { Metadata, Viewport } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains-mono",
  subsets: ["latin"],
});

export const viewport: Viewport = {
  themeColor: "#89b4fa",
  colorScheme: "dark",
};

export const metadata: Metadata = {
  title: "Beehive — Orchestrate Coding Agents",
  description:
    "A desktop app for orchestrating coding agents across isolated git workspaces. Manage repos, create branch-specific clones, and run terminals side-by-side.",
  metadataBase: new URL("https://www.beehiveapp.dev"),
  openGraph: {
    title: "Beehive — Orchestrate Coding Agents",
    description:
      "A desktop app for orchestrating coding agents across isolated git workspaces.",
    type: "website",
    url: "https://www.beehiveapp.dev",
    siteName: "Beehive",
  },
  twitter: {
    card: "summary_large_image",
    creator: "@technoleviathan",
    title: "Beehive — Orchestrate Coding Agents",
    description:
      "A desktop app for orchestrating coding agents across isolated git workspaces.",
  },
  icons: {
    icon: [
      { url: "/favicon.ico", sizes: "any" },
      { url: "/favicon-16x16.png", sizes: "16x16", type: "image/png" },
      { url: "/favicon-32x32.png", sizes: "32x32", type: "image/png" },
    ],
    apple: "/apple-touch-icon.png",
  },
  keywords: [
    "coding agents",
    "git workspaces",
    "terminal",
    "Claude Code",
    "developer tools",
    "macOS",
  ],
  authors: [{ name: "Mykyta Storozhenko", url: "https://storozh.dev" }],
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${inter.variable} ${jetbrainsMono.variable} font-sans antialiased`}
      >
        {children}
      </body>
    </html>
  );
}
