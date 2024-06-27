import { ImageZoom } from "fumadocs-ui/components/image-zoom"

interface Props {
  width: number
  height: number
  src: string
  className?: string
  alt?: string
  subtitle?: string
}

export default function ImageSection({ width, height, className, alt, src, subtitle }: Props) {
  return (
    <div className="[&_img]:m-0 mx-auto [&>span]:w-fit flex flex-col justify-start bg-neutral-100 dark:bg-neutral-900 border border-neutral-200 dark:border-neutral-800 rounded-lg p-2 max-w-2xl mb-4">
      <ImageZoom
        width={width}
        height={height}
        className={`mx-auto max-w-full w-auto h-fit min-w-content rounded-md ${className}`}
        alt={alt ?? ""}
        src={src}
      />
      {subtitle ? (
        <div className="mx-auto break-words whitespace-normal opacity-50 flex-shrink mt-2 text-center text-pretty text-sm">
          {subtitle}
        </div>
      ) : null}
    </div>
  )
}
