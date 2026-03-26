import type { NextConfig } from "next";

const isDev = process.env.NODE_ENV === "development";

const nextConfig: NextConfig = {
  // Production: static export for rust-embed embedding
  // Development: need rewrites to proxy API to backend
  ...(!isDev && { output: "export" }),
  ...(isDev && {
    async rewrites() {
      return [
        {
          source: "/api/:path*",
          destination: "http://localhost:3000/api/:path*",
        },
      ];
    },
  }),
};

export default nextConfig;
