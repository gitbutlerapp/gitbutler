import { DocsLayout } from "fumadocs-ui/layout"
import type { ReactNode } from "react"
import { docsOptions } from "@/app/layout.config"
import "fumadocs-ui/twoslash.css"

export default function Layout({ children }: { children: ReactNode }) {
  return <DocsLayout {...docsOptions}>{children}</DocsLayout>
}
