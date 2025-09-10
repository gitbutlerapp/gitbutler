import { readdir, readFile, writeFile } from "node:fs/promises"

const CONTENT_DIR = "./content/docs"

let fileContent = []
let filePaths = {}

try {
  const files = await readdir(CONTENT_DIR, {
    recursive: true,
    withFileTypes: true
  })

  for (const file of files) {
    if (file.isFile() && file.name.includes("mdx") && !file.parentPath.includes("api-reference")) {
      filePaths[file.name] = `${file.parentPath}/${file.name}`
    }
  }
  fileContent = `
# GitButler Documentation

## Table of Contents 

${Object.entries(filePaths).map((file) => {
  return `- [${file[0].replace(".mdx","")}](#${file[0]})\n`
}).join("")}`
} catch (err) {
  console.error("Error reading files", err)
}

await Promise.all(
  Object.entries(filePaths).map(async (file) => {
    let fileContents = (await readFile(`${file[1]}`)).toString()
    fileContents = fileContents.replace(/---.*---\n/s, "")
    fileContents = fileContents.replace(/import .*\n/g, "")

    fileContent += `\n\n# ${file[0]}`
    fileContent += `\n${fileContents}`
  })
)

await writeFile("./public/llms-full.txt", fileContent)
