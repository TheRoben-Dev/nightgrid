# DocuSeal document signing

Send PDFs for electronic signature and fill-in from your own server, instead of a paid e-signature service. Upload a PDF, drag signature, date and text fields onto it, and email signers a link. Handy for repair authorisations, rental forms or club paperwork.

## What you end up with

- DocuSeal behind your reverse proxy with its data on the host
- Mail set up so signers get their links and the signed copy
- A first template with fields and a signer role

## Before you start

- HTTPS on a real domain: signers open links from their email, so the site must be reachable for them.
- An SMTP relay for outgoing mail.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `docuseal`:

```yaml
services:
  docuseal:
    image: docuseal/docuseal:latest
    restart: unless-stopped
    volumes:
      - ./data:/data
    ports:
      - "127.0.0.1:3000:3000"
```

- [ ] Point the reverse proxy for `sign.example.com` at `127.0.0.1:3000`.
- [ ] Open the site and create the admin account on the first screen; do this right away, before anyone else finds the page.
- [ ] **Settings → Email**: your SMTP host, port, user and password, and a sender address. Send yourself a test.

## Part 2 · A template

- [ ] **Upload** a PDF (or a Word file), then place **Signature**, **Date**, **Text** and **Checkbox** fields on it and assign each to a party, such as "Customer".
- [ ] **Send** to an email address; the signer gets a link, fills the fields and signs. Everyone gets the completed PDF.
- [ ] Completed documents carry an audit log (who, when, IP) attached to the PDF.

#### Check

- [ ] Sign a test document from your phone using the emailed link and confirm the completed PDF opens with the signature and audit page.

## In Nightgrid

- Add the site to [Services](view:services) with **Expiry watch → Add…** so its certificate never lapses mid-signature.
- Back up the `data` folder: by default it holds the SQLite database and every uploaded and signed document.

## Limits and gotchas

- Whether an e-signature is legally valid for a given document depends on your country and the document type; DocuSeal records evidence but can't decide that for you.
- For more than a few users, point `DATABASE_URL` at PostgreSQL instead of the default SQLite.
- Some features (bulk send via API extras, SMS verification) are in the paid Pro edition; the core signing flow is free.

## Related

- [Stirling-PDF toolkit](kb:apps/stirling-pdf)
- [Paperless-ngx documents](kb:apps/paperless-ngx)
- [Send mail from your servers](kb:apps/mail-relay)
