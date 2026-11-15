import React from 'react';
import CodeBlock from '@theme/CodeBlock';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';

export default function AphynDependency({title, manifestHead = ''}) {
  const {siteConfig} = useDocusaurusContext();
  const {engineVersion, repoUrl} = siteConfig.customFields;
  return (
    <CodeBlock language="toml" title={title}>
      {`${manifestHead}[dependencies]\naphyn = { git = "${repoUrl}", tag = "v${engineVersion}" }`}
    </CodeBlock>
  );
}
