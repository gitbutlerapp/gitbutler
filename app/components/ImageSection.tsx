import { ImageZoom } from "fumadocs-ui/components/image-zoom"

interface Props {
  width: number
  height: number
  src: string
  alt?: string
  className?: string
  subtitle?: string
}

export default function ImageSection({ src, alt, subtitle }: Props) {
  return (
    <div className="mx-auto mb-4 flex flex-col justify-start rounded-lg border border-neutral-200 bg-neutral-100 p-2 dark:border-neutral-800 dark:bg-neutral-900 [&>span]:w-fit [&_img]:m-0">
      <ImageZoom
        width="0"
        height="0"
        className="h-auto w-full rounded-md"
        sizes="(min-width: 808px) 50vw, 100vw"
        alt={alt ?? subtitle ?? ""}
        src={src}
      />
      {subtitle ? (
        <div className="mx-auto mt-2 flex-shrink whitespace-normal text-pretty break-words text-center text-xs opacity-50">
          {subtitle}
        </div>
      ) : null}
    </div>
  )
}
