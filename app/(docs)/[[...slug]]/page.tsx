import { getPage, getPages } from "@/app/source"
import { DocsPage, DocsBody } from "fumadocs-ui/page"
import { notFound } from "next/navigation"
import { RollButton } from "fumadocs-ui/components/roll-button"
import type { Metadata } from "next"

export default async function Page({ params }: { params: { slug?: string[] } }) {
  const page = getPage(params.slug)

  if (!page) notFound()

  const footer = (
    <>
      <a
        href={`https://github.com/gitbutler/gitbutler-docs/blob/main/content/docs/${page.file.path}`}
        target="_blank"
        rel="noreferrer noopener"
        className="group rounded-md text-neutral-500 dark:text-neutral-400 dark:bg-neutral-900 border border-neutral-300/50 text-sm py-1 dark:border-neutral-700 flex justify-center items-center gap-2 hover:bg-neutral-100 transition duration-300 dark:hover:bg-neutral-950"
      >
        <svg
          className="size-4 group-hover:animate-[var(--animation-shake-x)]"
          xmlns="http://www.w3.org/2000/svg"
          width="32"
          height="32"
          fill="currentColor"
          viewBox="0 0 256 256"
        >
          <path d="M208.31,75.68A59.78,59.78,0,0,0,202.93,28,8,8,0,0,0,196,24a59.75,59.75,0,0,0-48,24H124A59.75,59.75,0,0,0,76,24a8,8,0,0,0-6.93,4,59.78,59.78,0,0,0-5.38,47.68A58.14,58.14,0,0,0,56,104v8a56.06,56.06,0,0,0,48.44,55.47A39.8,39.8,0,0,0,96,192v8H72a24,24,0,0,1-24-24A40,40,0,0,0,8,136a8,8,0,0,0,0,16,24,24,0,0,1,24,24,40,40,0,0,0,40,40H96v16a8,8,0,0,0,16,0V192a24,24,0,0,1,48,0v40a8,8,0,0,0,16,0V192a39.8,39.8,0,0,0-8.44-24.53A56.06,56.06,0,0,0,216,112v-8A58.14,58.14,0,0,0,208.31,75.68ZM200,112a40,40,0,0,1-40,40H112a40,40,0,0,1-40-40v-8a41.74,41.74,0,0,1,6.9-22.48A8,8,0,0,0,80,73.83a43.81,43.81,0,0,1,.79-33.58,43.88,43.88,0,0,1,32.32,20.06A8,8,0,0,0,119.82,64h32.35a8,8,0,0,0,6.74-3.69,43.87,43.87,0,0,1,32.32-20.06A43.81,43.81,0,0,1,192,73.83a8.09,8.09,0,0,0,1,7.65A41.72,41.72,0,0,1,200,104Z"></path>
        </svg>
        Edit on GitHub
      </a>
      <a
        href={`https://github.com/gitbutler/gitbutler-docs/issues/new?label=docs&title=Feedback+for+page+"${page.file.flattenedPath}"`}
        target="_blank"
        rel="noreferrer noopener"
        className="rounded-md text-neutral-500 dark:text-neutral-400 dark:bg-neutral-900 border border-neutral-300/50 text-sm py-1 dark:border-neutral-700 flex justify-center items-center gap-2 hover:bg-neutral-100 transition duration-300 dark:hover:bg-neutral-950 group"
      >
        <svg
          className="size-4 transition ease-[var(--ease-spring-3)] duration-500 group-hover:animate-[var(--animation-bounce)]"
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 256 256"
        >
          <rect width="256" height="256" fill="none" />
          <path
            d="M32,104H80a0,0,0,0,1,0,0V208a0,0,0,0,1,0,0H32a8,8,0,0,1-8-8V112A8,8,0,0,1,32,104Z"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="16"
          />
          <path
            d="M80,104l40-80a32,32,0,0,1,32,32V80h64a16,16,0,0,1,15.87,18l-12,96A16,16,0,0,1,204,208H80"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="16"
          />
        </svg>
        Give us feedback
      </a>
    </>
  )

  const MDX = page.data.exports.default

  return (
    <DocsPage
      toc={page.data.exports.toc}
      full={page.data.full ?? false}
      tableOfContent={{
        footer
      }}
      tableOfContentPopover={{ footer }}
    >
      <RollButton percentage={0.3} />
      <DocsBody>
        <h1>{page.data.title}</h1>
        <MDX />
      </DocsBody>
    </DocsPage>
  )
}

export async function generateStaticParams() {
  return getPages().map((page) => ({
    slug: page.slugs
  }))
}

export function generateMetadata({ params }: { params: { slug?: string[] } }) {
  const page = getPage(params.slug)

  if (page == null) notFound()

  return {
    title: page.data.title,
    description: page.data.description
  } satisfies Metadata
}
