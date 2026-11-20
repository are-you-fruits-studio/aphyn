/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Getting started',
      collapsible: false,
      items: ['getting-started', 'build-a-game'],
    },
    {
      type: 'category',
      label: 'Concepts',
      collapsible: false,
      items: ['concepts/architecture', 'concepts/ecs'],
    },
  ],
};

module.exports = sidebars;
