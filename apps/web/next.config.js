/** @type {import('next').NextConfig} */
const nextConfig = {
  experimental: {
    proxyClientMaxBodySize: Number.MAX_SAFE_INTEGER,
  },
};

module.exports = nextConfig;
