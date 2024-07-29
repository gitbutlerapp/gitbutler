import { generateFiles } from "fumadocs-openapi"
import { writeFile } from "node:fs/promises"

// Fetch and Convert GitButler's v2 OpenAPI Spec to v3
const rawGitButlerSwagger = "https://app.gitbutler.com/api/swagger_doc.json"
const gitButlerSwaggerConverted = `https://converter.swagger.io/api/convert?url=${encodeURIComponent(rawGitButlerSwagger)}`

const swaggerResponse = await fetch(gitButlerSwaggerConverted)
const swaggerContent = await swaggerResponse.json()

// Modify a few entries to avoid parsing bugs and polish up the displaying
if (swaggerContent.servers?.[0]?.url === "//app.gitbutler.com/api") {
  swaggerContent.servers[0].url = "https://app.gitbutler.com/api"
}

if (swaggerContent.info.title === "API title") {
  swaggerContent.info.title = "GitButler API"
}

await writeFile("./api-reference.json", JSON.stringify(swaggerContent, null, 2))

function toTitleCase(str) {
  return str
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ")
}

void generateFiles({
  input: ["./api-reference.json"],
  per: "tag",
  frontmatter: (title, description) => {
    return {
      title: toTitleCase(title),
      description
    }
  },
  output: "./content/docs/"
})
