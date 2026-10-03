---
layout: home

hero:
  name: openbim-ifcx
  text: IFC5 / IFCX for Rust, JavaScript and Python
  tagline: Read and write IFCX files losslessly, compose their layers, validate attributes against their schemas, and export the model as GLB. Pure Rust, no unsafe, the same core in every language.
  image:
    src: /logo.svg
    alt: openbim-ifcx
  actions:
    - theme: brand
      text: Open the viewer
      link: /viewer/
      target: _self
    - theme: alt
      text: Get started
      link: /guide/install
    - theme: alt
      text: View on GitHub
      link: https://github.com/openbimrs/ifcx

features:
  - title: Lossless round trip
    details: Key order, fields the draft does not define, null deletions and every number come back exactly as read. All 47 upstream example files round-trip.
    link: /capabilities
    linkText: Capabilities
  - title: Composition as upstream does it
    details: Layers flatten by path and compose into a tree, imports resolve through a resolver you choose, and the composed trees equal upstream's TypeScript for every example.
    link: /evidence
    linkText: Upstream parity and drift
  - title: Geometry without a renderer
    details: Transforms, meshes, curves, point clouds and materials decode into a flat render scene with f64 precision, exported as GLB for any glTF viewer.
    link: /reference/crates/openbim-ifcx-geometry
    linkText: openbim-ifcx-geometry
  - title: One core, three languages
    details: The crates on crates.io, @openbim/ifcx on npm for Node, bundlers and browsers, and openbim-ifcx on PyPI share one binding core and one set of error codes.
    link: /guide/install
    linkText: Install
---

## Try it in the browser

The [viewer](/viewer/){target="_self"} composes an `.ifcx` file in your
browser with `@openbim/ifcx`, exports it as GLB and shows it with three.js,
with the node tree, picked attributes and validation failures. Files stay
on your machine. It works on phones too: tap a part to pick it. See
[the viewer guide](/guide/viewer) for what it does.

## Install

::: code-group

```sh [Rust]
cargo add openbim-ifcx openbim-ifcx-geometry
```

```sh [JavaScript]
npm install @openbim/ifcx
```

```sh [Python]
pip install openbim-ifcx
```

:::

[Install](/guide/install) lists every package with its current release, then
the guides for [Rust](/guide/rust), [JavaScript](/guide/javascript) and
[Python](/guide/python) take a file from reading to GLB.
