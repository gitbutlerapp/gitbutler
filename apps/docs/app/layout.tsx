import "./global.css"
import { Provider } from "./provider"
import { Inter } from "next/font/google"
import type { Metadata, Viewport } from "next"
import type { ReactNode } from "react"
import Script from "next/script"

const baseUrl =
  process.env.NODE_ENV === "development"
    ? new URL("http://localhost:3000")
    : new URL(`https://${process.env.VERCEL_URL}`)

const inter = Inter({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-inter"
})

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className={`${inter.variable}`} suppressHydrationWarning>
      <body>
        <Provider>{children}</Provider>
      </body>
      <Script
        async
        src="https://u.gitbutler.com/script.js"
        data-website-id="2f6dbf62-091f-4e57-bc47-7b1c6611a98b"
      />
    </html>
  )
}

export const metadata: Metadata = {
  title: {
    template: "%s | GitButler Docs",
    default: "GitButler Docs"
  },
  description:
    "GitButler is a new Source Code Management system designed to manage your branches, record and backup your work, be your Git client, help with your code and much more",
  openGraph: {
    images: "/cover.png",
    title: {
      template: "%s | GitButler Docs",
      default: "GitButler Docs"
    },
    url: "https://docs.gitbutler.com",
    siteName: "GitButler Docs",
    description:
      "GitButler is a new Source Code Management system designed to manage your branches, record and backup your work, be your Git client, help with your code and much more"
  },
  twitter: {
    card: "summary_large_image",
    creator: "@gitbutler",
    title: "GitButler Docs",
    description:
      "GitButler is a new Source Code Management system designed to manage your branches, record and backup your work, be your Git client, help with your code and much more",
    images: "/cover.png"
  },
  metadataBase: baseUrl,
  applicationName: "GitButler Docs",
  robots: {
    index: true,
    follow: true
  },
  icons: { icon: "/fav/fav-64.png", apple: "/fav/fav-180.png" }
}

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: dark)", color: "#707070" },
    { media: "(prefers-color-scheme: light)", color: "#f5f5f3" }
  ],
  colorScheme: "dark light",
  width: "device-width",
  initialScale: 1
}
