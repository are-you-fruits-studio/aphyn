const {themes} = require('prism-react-renderer');

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'aphyn',
  tagline: 'Rust game engine documentation',
  url: 'https://are-you-fruits-studio.github.io',
  baseUrl: '/aphyn/',
  favicon: 'img/favicon.png',
  organizationName: 'are-you-fruits-studio',
  projectName: 'aphyn',
  trailingSlash: false,
  onBrokenLinks: 'warn',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: 'docs',
          sidebarPath: require.resolve('./sidebars.js'),
          showLastUpdateTime: true,
        },
        blog: false,
        theme: {
          customCss: require.resolve('./src/css/custom.css'),
        },
      },
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      navbar: {
        title: 'aphyn',
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'docsSidebar',
            position: 'left',
            label: 'Docs',
          },
          {
            // Raw <a> so the browser does a full navigation to the static rustdoc
            // subtree. A normal navbar `href` item is wrapped in Docusaurus's
            // client-side <Link>, which treats this baseUrl-relative path as an
            // internal route, fails to match it, and renders the SPA 404.
            type: 'html',
            position: 'left',
            value: '<a class="navbar__item navbar__link" href="/aphyn/api-reference/aphyn/index.html">API Reference</a>',
          },
          {
            href: 'https://github.com/are-you-fruits-studio/aphyn',
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Documentation',
            items: [
              {
                label: 'Docs',
                to: '/docs/getting-started',
              },
            ],
          },
          {
            title: 'Project',
            items: [
              {
                label: 'GitHub',
                href: 'https://github.com/are-you-fruits-studio/aphyn',
              },
            ],
          },
        ],
        copyright: `Copyright (c) ${new Date().getFullYear()} Are You Fruits?.`,
      },
      prism: {
        theme: themes.github,
        darkTheme: themes.dracula,
        additionalLanguages: ['rust', 'toml'],
      },
    }),
};

module.exports = config;
