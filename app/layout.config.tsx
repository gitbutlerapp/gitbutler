import { type BaseLayoutProps, type DocsLayoutProps } from "fumadocs-ui/layout"
import { pageTree } from "./source"
import Logo from "@/components/Logo"
import Discord from "@/components/logos/discord"
import GitButler from "@/components/logos/gitbutler-wordmark"

// shared configuration
export const baseOptions: BaseLayoutProps = {
  nav: {
    title: <Logo />,
    transparentMode: "top"
  },
  githubUrl: "https://github.com/gitbutlerapp/gitbutler",
  links: [
    {
      icon: <Discord />,
      text: "Discord",
      url: "https://discord.com/invite/MmFkmaJ42D"
    },
    {
      icon: <GitButler />,
      text: "GitButler Cloud",
      url: "https://app.gitbutler.com/"
    }
  ]
}

// docs layout configuration
export const docsOptions: DocsLayoutProps = {
  ...baseOptions,
  sidebar: {
    defaultOpenLevel: 0
  },
  tree: pageTree
}
