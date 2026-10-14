---
title: Reading assets, starting with DBPF
description: The relationship between TGI indices, RefPack and asset payloads.
research: true
---
# Reading assets, starting with DBPF

> Format primer · Documentation summary · Compiled on 2026-10-15. Based on the project's format reference and README. Sample coverage is not a guarantee for every game installation.

A `.package` is an asset container, not a single model or image format. Start by separating “finding the bytes” from “understanding the bytes.”

## The container locates resources

The DBPF index records identifiers, offsets, sizes and compression information. **TGI** consists of Type, Group and Instance and identifies resources and their references.

| Field | Question it answers |
| --- | --- |
| Type | What kind of resource is this? |
| Group | Which group does it belong to? |
| Instance | Which particular instance is this? |

## Compression restores the payload

When an entry is marked as RefPack-compressed, decompress it before passing it to the relevant parser. Validate the index, decompressed bytes and internal resource structure separately so failures are attributed to the correct layer.

## Resource types determine the next step

- **Property**: values and references linking models, lots and simulation parameters.
- **RW4**: section structures for models, textures, skeletons and animations.
- **Raster**: bitmap data and metadata, potentially requiring a specific pixel-format decoder.
- **Text and media**: text can be inspected directly; some audio and video need external decoders.

## Names help; they do not prove a format

A descriptor database maps numeric IDs to readable names. The archived research also records mismatches between labels and actual content. Some resources labeled Javascript File are actually UI layout JSON.

Names, labels and extensions are starting points. Confirm the interpretation using headers, structures, references and parsing results.

## Evidence and limits

- Original document: `docs/overview/file-formats.md`, whose format census is dated 2026-09-09.
- This article explains layers and identification methods, not a parser support matrix that may change between versions.
- [Read the archived format reference, in Chinese](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/overview/file-formats.md).

Ready to inspect resources? [Get started](../guide/getting-started).
