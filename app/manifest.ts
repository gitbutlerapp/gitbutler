import { MetadataRoute } from "next"

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "GitButler",
    short_name: "GitButler",
    description:
      "GitButler is a new Source Code Management system designed to manage your branches, record and backup your work, be your Git client, help with your code and much more",
    start_url: "/",
    theme_color: "#707070",
    background_color: "#707070",
    display: "standalone",
    icons: [
      {
        src: "fav/fav-32.png",
        sizes: "32x32",
        type: "image/png"
      },
      {
        src: "fav/fav-64.png",
        sizes: "64x64",
        type: "image/png"
      },
      {
        src: "fav/fav-180.png",
        sizes: "180x180",
        type: "image/png"
      },
      {
        src: "fav/fav-svg.svg",
        type: "image/svg+xml"
      }
    ]
  }
}
