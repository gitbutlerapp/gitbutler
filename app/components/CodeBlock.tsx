import * as Base from "fumadocs-ui/components/codeblock"
import type { HTMLAttributes } from "react"
import { useMemo } from "react"
import { getHighlighter } from "shiki"

const highlighter = await getHighlighter({
  langs: ["bash", "ts", "tsx"],
  themes: ["github-light", "github-dark"]
})

export type CodeBlockProps = HTMLAttributes<HTMLPreElement> & {
  code: string
  wrapper?: Base.CodeBlockProps
  lang: "bash" | "ts" | "tsx"
}

export function CodeBlock({ code, lang, wrapper, ...props }: CodeBlockProps): React.ReactElement {
  const html = useMemo(
    () =>
      highlighter.codeToHtml(code, {
        lang,
        defaultColor: false,
        themes: {
          light: "github-light",
          dark: "github-dark"
        },
        transformers: [
          {
            name: "remove-pre",
            root: (root) => {
              if (root.children[0].type !== "element") return

              return {
                type: "root",
                children: root.children[0].children
              }
            }
          }
        ]
      }),
    [code, lang]
  )

  return (
    <Base.CodeBlock {...wrapper}>
      <Base.Pre {...props} dangerouslySetInnerHTML={{ __html: html }} />
    </Base.CodeBlock>
  )
}
