import { Code, Root, RootContent } from "mdast"
import { Plugin } from "unified"
import { visit } from "unist-util-visit"

const COMPONENT_NAME = "Mermaid"

const MERMAID_IMPORT_AST = {
  type: "mdxjsEsm",
  data: {
    estree: {
      body: [
        {
          type: "ImportDeclaration",
          specifiers: [
            {
              type: "ImportSpecifier",
              imported: { type: "Identifier", name: COMPONENT_NAME },
              local: { type: "Identifier", name: COMPONENT_NAME }
            }
          ],
          source: { type: "Literal", value: "@/components/mermaid/Mermaid" }
        }
      ]
    }
  }
} as RootContent

export const remarkMermaid: Plugin<[], Root> = () => (ast, _file, done) => {
  // eslint-disable-next-line
  const codeblocks: any[] = []
  visit(ast, { type: "code", lang: "mermaid" }, (node: Code, index, parent) => {
    codeblocks.push([node, index, parent])
  })

  if (codeblocks.length !== 0) {
    for (const [node, index, parent] of codeblocks) {
      parent.children.splice(index, 1, {
        type: "mdxJsxFlowElement",
        name: COMPONENT_NAME,
        attributes: [
          {
            type: "mdxJsxAttribute",
            name: "chart",
            value: node.value.replaceAll("\n", "\\n")
          }
        ]
      })
    }
    ast.children.unshift(MERMAID_IMPORT_AST)
  }

  done()
}
