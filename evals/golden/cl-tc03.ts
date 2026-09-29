#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// thinking-tester-contact-list — edit + delete: the destructive half of
// CRUD. Delete fires a native confirm() synchronously in the handler, so
// the sweep keeps dialogs pending and asserts its text before accepting.
await runEdgeGolden(
  "tc03", "contact-list — edit contact, then delete via confirm",
  "https://thinking-tester-contact-list.herokuapp.com/addUser", "#firstName",
  async (b) => {
    const stamp = Date.now();
    const email = `golden-${stamp}@example.com`;
    await b.openPage();
    await b.fillSelector("#firstName", "Golden", "type first name");
    await b.fillSelector("#lastName", "Runner", "type last name");
    await b.fillSelectorReplayValue("#email", email, "golden-{{vars._unique}}@example.com", "type a fresh signup email");
    await b.fillSelector("#password", "golden-pass-123", "type a password");
    await b.clickSelector("#submit", "submit the signup");
    await b.waitSelector("#add-contact", "contact list rendered");
    await b.waitMs(700, "let the SPA wire its handlers");
    await b.clickSelector("#add-contact", "open the add-contact form");
    await b.waitSelector("#birthdate", "add-contact form rendered");
    await b.fillSelector("#firstName", "Grace", "type the contact's first name");
    await b.fillSelector("#lastName", "Hopper", "type the contact's last name");
    await b.fillSelector("#email", "grace.hopper@example.com", "type the contact's email");
    await b.clickSelector("#submit", "submit the contact");
    await b.waitSelectorText(".contactTable", "Grace Hopper", "contact row appears");
    await b.waitMs(500, "let the row listener attach");
    await b.clickSelector(".contactTableBodyRow", "open the contact row");
    await b.waitSelector("#edit-contact", "details rendered");
    await b.waitMs(700, "let the details handlers bind");
    await b.clickSelector("#edit-contact", "open the edit form");
    await b.waitSelector("#submit", "edit form rendered");
    await b.waitMs(700, "let the edit form bind");
    await b.fillSelector("#lastName", "Hopper-Edit", "change the last name");
    await b.clickSelector("#submit", "submit the edit");
    await b.waitSelector("#return", "back on the details page");
    await b.assertElementAttribute("#lastName", "text", "equals", "Hopper-Edit", "details show the edited name");
    await b.waitMs(700, "let the delete handler bind");
    await b.clickSelector("#delete", "delete the contact");
    await b.assertDialogText("delete", "delete confirm fired");
    await b.dialogAccept("confirm the delete");
    await b.assertDialogClosed("confirm dismissed");
    await b.waitSelector("#add-contact", "back on the list");
  },
  { label: "cl", keepDialogs: true },
);
