import React from 'react';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';

export default function Home() {
  return (
    <Layout
      title="Fruits Engine"
      description="Rust game engine documentation"
    >
      <main className="home">
        <section className="home__hero">
          <div className="container">
            <p className="home__eyebrow">Are You Fruits?</p>
            <h1>Fruits Engine</h1>
            <p className="home__intro">
              Documentation for engine concepts and API reference.
            </p>
            <div className="home__actions">
              <Link className="button button--primary" to="/docs/getting-started">
                Open Docs
              </Link>
              <a
                className="button button--secondary"
                href="/aphyn/api-reference/aphyn/index.html"
              >
                View API Reference
              </a>
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
