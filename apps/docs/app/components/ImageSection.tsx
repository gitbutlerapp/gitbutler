import { ImageZoom } from "fumadocs-ui/components/image-zoom"

interface Props {
  /**
   * Image path relative to `/public/img/docs`
   */
  src: string
  alt?: string
  className?: string
  subtitle?: string
}

const shimmer = (w: number, h: number) => `
<svg width="${w}" height="${h}" version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
  <defs>
    <linearGradient id="g">
      <stop stop-color="#333" offset="20%" />
      <stop stop-color="#222" offset="50%" />
      <stop stop-color="#333" offset="70%" />
    </linearGradient>
  </defs>
  <rect width="${w}" height="${h}" fill="#333" />
  <rect id="r" width="${w}" height="${h}" fill="url(#g)" />
  <animate xlink:href="#r" attributeName="x" from="-${w}" to="${w}" dur="1s" repeatCount="indefinite"  />
</svg>`

const toBase64 = (str: string) =>
  typeof window === "undefined" ? Buffer.from(str).toString("base64") : window.btoa(str)

export default async function ImageSection({ src, alt, subtitle }: Props) {
  const img = await import(`../../public/img/docs${src}`).then((mod) => mod.default)
  if (!img) return null

  return (
    <div className="mx-auto mb-4 flex flex-col justify-start rounded-lg border border-neutral-200 bg-neutral-100 p-2 dark:border-neutral-800 dark:bg-neutral-900 [&>span]:w-fit [&_img]:m-0">
      <ImageZoom
        className="rounded-md"
        placeholder={`data:image/svg+xml;base64,${toBase64(shimmer(img.width, img.height))}`}
        alt={alt ?? subtitle ?? ""}
        src={img}
      />
      {subtitle ? (
        <div className="mx-auto mt-2 flex-shrink whitespace-normal text-pretty break-words text-center text-xs opacity-50">
          {subtitle}
        </div>
      ) : null}
    </div>
  )
}
