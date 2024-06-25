import "./global.css"
import { Provider } from "./provider"
import localFont from "next/font/local"
import type { Metadata, Viewport } from "next"
import type { ReactNode } from "react"

const SITE_URL = process.env.SITE_URL ?? "http://localhost:3000"
const urlBase = new URL(SITE_URL)

const ppEditorialNew = localFont({
  src: [
    {
      path: "../public/fonts/PPEditorialNew-Regular.woff2",
      weight: "300"
    },
    {
      path: "../public/fonts/PPEditorialNew-Italic.woff2",
      weight: "300",
      style: "italic"
    }
  ],
  variable: "--font-ppeditorialnew"
})

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html
      lang="en"
      className={`${ppEditorialNew.variable}`}
      suppressHydrationWarning
    >
      <body>
        <Provider>{children}</Provider>
      </body>
    </html>
  )
}

export const metadata: Metadata = {
  title: {
    template: "%s | GitButler",
    default: "GitButler"
  },
  description:
    "GitButler is a new Source Code Management system designed to manage your branches, record and backup your work, be your Git client, help with your code and much more",
  twitter: {
    card: "summary_large_image"
  },
  openGraph: {
    images: "/cover.png",
    title: {
      template: "%s | GitButler",
      default: "GitButler"
    },
    description:
      "GitButler is a new Source Code Management system designed to manage your branches, record and backup your work, be your Git client, help with your code and much more"
  },
  metadataBase: urlBase,
  applicationName: "GitButler",
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
