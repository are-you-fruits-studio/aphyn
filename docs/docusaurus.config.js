const fs = require('fs');
const path = require('path');
const {themes} = require('prism-react-renderer');

const repoUrl = 'https://github.com/are-you-fruits-studio/aphyn';

function readEngineVersion() {
  const manifest = fs.readFileSync(path.join(__dirname, '..', 'Cargo.toml'), 'utf8');
  const match = manifest.match(/^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error('No version under [workspace.package] in the root Cargo.toml');
  }
  return match[1];
}

const engineVersion = readEngineVersion();

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'Aphyn',
  tagline: 'Documentation for the Aphyn game engine',
  url: 'https://docs.aphyn.org',
  baseUrl: '/',
  favicon: 'img/favicon.ico',
  headTags: [
    {tagName: 'link', attributes: {rel: 'icon', type: 'image/svg+xml', href: '/img/favicon.svg'}},
    {tagName: 'link', attributes: {rel: 'apple-touch-icon', href: '/img/apple-touch-icon.png'}},
  ],
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
  customFields: {
    engineVersion,
    repoUrl,
  },

  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: 'docs',
          sidebarPath: require.resolve('./sidebars.js'),
          editUrl: `${repoUrl}/edit/dev/docs/`,
          showLastUpdateTime: true,
        },
        blog: false,
        theme: {
          customCss: require.resolve('./src/css/custom.css'),
        },
      },
    ],
  ],

  themes: [
    [
      require.resolve('@easyops-cn/docusaurus-search-local'),
      {
        hashed: true,
        indexBlog: false,
        indexPages: false,
        docsRouteBasePath: 'docs',
        language: ['en'],
        searchBarShortcutHint: true,
      },
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      navbar: {
        title: 'Aphyn',
        logo: {
          alt: 'Aphyn',
          src: 'img/favicon.svg',
        },
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'docsSidebar',
            position: 'left',
            label: 'Guide',
          },
          {
            // Raw <a> so the browser does a full navigation to the static rustdoc
            // subtree. A normal navbar `href` item is wrapped in Docusaurus's
            // client-side <Link>, which treats this baseUrl-relative path as an
            // internal route, fails to match it, and renders the SPA 404.
            type: 'html',
            position: 'left',
            value: '<a class="navbar__item navbar__link" href="/api-reference/aphyn/index.html">API Reference</a>',
          },
          {
            type: 'search',
            position: 'right',
          },
          {
            type: 'html',
            position: 'right',
            value: `<a class="navbar__item aphyn-version" href="${repoUrl}/releases" title="Engine version on dev">v${engineVersion}</a>`,
          },
          {
            href: 'https://aphyn.org',
            label: 'aphyn.org',
            position: 'right',
          },
          {
            href: repoUrl,
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Docs',
            items: [
              {label: 'Quick start', to: '/docs/getting-started'},
              {label: 'Build a game', to: '/docs/build-a-game'},
              {
                html: '<a class="footer__link-item" href="/api-reference/aphyn/index.html">API Reference</a>',
              },
            ],
          },
          {
            title: 'Project',
            items: [
              {label: 'aphyn.org', href: 'https://aphyn.org'},
              {label: 'Examples', href: 'https://github.com/are-you-fruits-studio/aphyn.examples'},
              {label: 'Releases', href: `${repoUrl}/releases`},
            ],
          },
          {
            title: 'Community',
            items: [
              {label: 'GitHub', href: repoUrl},
              {label: 'Issues', href: `${repoUrl}/issues`},
              {label: 'Contributing', href: `${repoUrl}/blob/dev/CONTRIBUTING.md`},
            ],
          },
        ],
        copyright: `© ${new Date().getFullYear()} are-you-fruits-studio · MIT OR Apache-2.0`,
      },
      prism: {
        theme: themes.github,
        darkTheme: themes.dracula,
        additionalLanguages: ['rust', 'toml', 'bash'],
      },
    }),
};

module.exports = config;
