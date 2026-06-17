import type { EvalCase } from "./lib/harness.ts";
import { textOf } from "./lib/harness.ts";

interface PracticePage {
  slug: string;
  label: string;
  path: string;
  smokeTerms: string[];
  cases: string[];
}

interface DetailedCase {
  pageSlug: string;
  tc: string;
  prompt: string;
  extraConstraints?: string[];
}

const practicePages: PracticePage[] = [
  {
    slug: "bank",
    label: "Bank Login",
    path: "/bank",
    smokeTerms: ["SecureBank", "Login"],
    cases: [
      "TC-LOGIN-01: Successful login with admin credentials",
      "TC-LOGIN-02: Failed login shows error alert for invalid credentials",
      "TC-LOGIN-03: Toggle password visibility hides and reveals password text",
      "TC-LOGIN-04: Pressing Enter in the password field submits the login form",
      "TC-LOGIN-05: Read-only viewer login grants restricted access",
    ],
  },
  {
    slug: "input-fields",
    label: "Input Fields",
    path: "/practice/input-fields",
    smokeTerms: ["input field", "movie name"],
    cases: [
      "TC01: Verify successful movie name input",
      "TC02: Verify input placeholder disappears on typing",
      "TC03: Verify keyboard tab triggers focus change after append",
      "TC04: Verify appended text value is retained in the field",
      "TC05: Verify text present inside input field matches expected value",
      "TC06: Verify getAttribute returns the correct input value",
      "TC07: Verify input field text can be cleared successfully",
      "TC08: Verify field is empty after executing clear action",
      "TC09: Verify disabled input field cannot be edited by user",
      "TC10: Verify isEnabled() returns false for disabled input",
      "TC11: Verify readonly input field does not accept user typing",
      "TC12: Verify getAttribute returns correct readonly attribute value",
    ],
  },
  {
    slug: "buttons",
    label: "Buttons",
    path: "/practice/buttons",
    smokeTerms: ["button", "double click"],
    cases: [
      "TC01: Verify button is clickable and triggers action",
      "TC02: Verify button displays the correct label text",
      "TC03: Verify button triggers the correct action on click",
      "TC04: Verify double-click button triggers double-click action",
      "TC05: Verify right-click button triggers context menu action",
      "TC06: Verify disabled button cannot be clicked",
      "TC07: Verify button is enabled when it should be",
      "TC08: Verify button is responsive on different screen sizes",
      "TC09: Verify button is accessible via keyboard",
      "TC10: Verify button is accessible to screen readers",
      "TC11: Verify button hover state is visually distinct",
      "TC12: Verify button state resets after page refresh",
      "TC13: Verify button does not overlap other page elements",
      "TC14: Verify button styling matches design specification",
      "TC15: Verify button page loads without errors",
    ],
  },
  {
    slug: "forms",
    label: "Forms",
    path: "/practice/forms",
    smokeTerms: ["form", "first name"],
    cases: [
      "TC01: Fill all fields with valid data and submit successfully",
      "TC02: Verify required field errors appear on empty submit",
      "TC03: Verify invalid email format shows validation error",
      "TC04: Verify invalid phone number format shows error",
      "TC05: Verify password minimum length validation",
      "TC06: Verify password mismatch shows confirm password error",
      "TC07: Verify T&C checkbox required error appears",
      "TC08: Verify success message displays submitted name",
      "TC09: Verify reset button clears all fields",
      "TC10: Verify gender radio button selection",
      "TC11: Verify country dropdown selection",
      "TC12: Verify multiple interest checkboxes can be selected",
      "TC13: Verify form fields retain values after validation failure",
      "TC14: Verify Fill Again button returns to empty form from success state",
      "TC15: Verify form page loads without errors",
    ],
  },
  {
    slug: "dropdowns",
    label: "Dropdowns",
    path: "/practice/dropdowns",
    smokeTerms: ["dropdown", "select fruit"],
    cases: [
      "TC01: Select 'Apple' from fruit dropdown by visible text",
      "TC02: Select 'India' from country dropdown by value attribute",
      "TC03: Verify selected value is displayed after selection",
      "TC04: Get all available options from the programming language dropdown",
      "TC05: Select the last option from the programming language dropdown",
      "TC06: Multi-select: select multiple superheroes using CTRL+click",
      "TC07: Multi-select: deselect a previously selected option",
      "TC08: Verify default placeholder text before any selection",
      "TC09: Verify a dropdown is enabled and interactable",
      "TC10: Verify the total count of options in the country dropdown",
    ],
  },
  {
    slug: "data-table",
    label: "Data Table",
    path: "/practice/data-table",
    smokeTerms: ["data table", "book name"],
    cases: [
      "TC01: Verify all table column headers are present",
      "TC02: Count the total number of rows in the data table",
      "TC03: Read a cell value from a specific row and column",
      "TC04: Find a book row by author name using XPath or filter",
      "TC05: Verify the table is not empty after page load",
      "TC06: Assert the ISBN column contains only string values",
    ],
  },
  {
    slug: "alerts-dialogs",
    label: "Alerts & Dialogs",
    path: "/practice/alerts-dialogs",
    smokeTerms: ["alerts", "dialogs"],
    cases: [
      "TC01: Accept a simple browser alert and verify it closes",
      "TC02: Get text from a simple browser alert before accepting",
      "TC03: Accept a confirm dialog and verify accepted state",
      "TC04: Dismiss a confirm dialog and verify dismissed state",
      "TC05: Enter text in a prompt dialog and accept it",
      "TC06: Dismiss a prompt dialog and verify no input is captured",
      "TC07: Verify toast notification appears after triggering",
      "TC08: Close a modal/sweet alert using the Cancel button",
      "TC09: Close an advanced dialog using the Close button",
      "TC10: Verify alerts page loads without errors",
    ],
  },
  {
    slug: "radio-checkbox",
    label: "Radio & Checkbox",
    path: "/practice/radio-checkbox",
    smokeTerms: ["radio", "checkbox"],
    cases: [
      "TC01: Verify radio button is selected on click",
      "TC02: Verify selecting another radio deselects the previous one",
      "TC03: Verify only one radio button can be selected at a time",
      "TC04: Verify radio button label text is correct",
      "TC05: Verify radio button state persists after page interaction",
      "TC06: Verify checkbox can be checked",
      "TC07: Verify checkbox can be unchecked",
      "TC08: Verify multiple checkboxes can be selected simultaneously",
      "TC09: Verify radio buttons are keyboard navigable",
      "TC10: Verify checkbox is keyboard togglable",
      "TC11: Verify disabled radio button cannot be selected",
      "TC12: Verify disabled checkbox cannot be toggled",
      "TC13: Verify radio button group is accessible to screen readers",
      "TC14: Verify radio button visual state changes on selection",
      "TC15: Verify radio and checkbox elements load without errors",
    ],
  },
  {
    slug: "date-picker",
    label: "Date Picker",
    path: "/practice/date-picker",
    smokeTerms: ["date picker", "date"],
    cases: [
      "TC01: Fill today's date in the date input and verify the value",
      "TC02: Enter a birthday date and assert the value is stored",
      "TC03: Fill a date range - start date and end date",
      "TC04: Verify date input rejects out-of-range date (min/max constraint)",
      "TC05: Clear a date input and verify it becomes empty",
    ],
  },
  {
    slug: "links",
    label: "Links",
    path: "/practice/links",
    smokeTerms: ["links", "navigation"],
    cases: [
      "TC01: Verify link navigates to the correct URL on click",
      "TC02: Verify link text matches expected label",
      "TC03: Verify external link opens in a new tab",
      "TC04: Verify internal link stays in the same tab",
      "TC05: Verify broken link returns HTTP error status",
      "TC06: Verify link is keyboard accessible",
      "TC07: Verify link href attribute contains the correct URL",
      "TC08: Verify link has accessible label for screen readers",
      "TC09: Verify link hover state is visually distinct",
      "TC10: Verify right-click on link shows browser context menu",
      "TC11: Verify link with dynamic URL resolves correctly",
      "TC12: Verify link page loads without console errors",
    ],
  },
  {
    slug: "tabs-windows",
    label: "Tabs & Windows",
    path: "/practice/tabs-windows",
    smokeTerms: ["tabs", "windows"],
    cases: [
      "TC01: Open a link in a new tab and switch to it",
      "TC02: Open multiple windows and print all window titles",
      "TC03: Switch back to the parent window after switching to child",
      "TC04: Close the child window and verify focus returns to parent",
      "TC05: Verify Ctrl+click opens a link in a new tab",
    ],
  },
  {
    slug: "dynamic-waits",
    label: "Dynamic Waits",
    path: "/practice/dynamic-waits",
    smokeTerms: ["dynamic waits", "wait"],
    cases: [
      "TC01: Wait for a delayed browser alert to appear and accept it",
      "TC02: Wait for a hidden element to become visible after a delay",
      "TC03: Wait for a disabled button to become enabled",
      "TC04: Wait for loading text to change to a loaded state",
      "TC05: Wait for a spinner to disappear before asserting completion",
    ],
  },
  {
    slug: "multi-select",
    label: "Multi Select",
    path: "/practice/multi-select",
    smokeTerms: ["multi-select", "select"],
    cases: [
      "TC01: Select multiple fruits using Ctrl+click in a native multi-select",
      "TC02: Deselect a specific option from a pre-selected multi-select",
      "TC03: Select all countries using the Select All button",
      "TC04: Check multiple checkboxes and verify selected output",
      "TC05: Add a tag and then remove it from the chip-based multi-select",
    ],
  },
  {
    slug: "file-upload",
    label: "File Upload",
    path: "/practice/file-upload",
    smokeTerms: ["file upload", "upload"],
    cases: [
      "Upload TC01: Verify a file can be selected for upload",
      "Upload TC02: Verify selected file name is displayed after selection",
      "Upload TC03: Verify upload button is enabled after file selection",
      "Upload TC04: Verify file upload starts on clicking upload button",
      "Upload TC05: Verify success message appears after upload",
      "Upload TC06: Verify error message for unsupported file type",
      "Upload TC07: Verify error message for files exceeding size limit",
      "Upload TC08: Verify uploaded file appears in the file list",
      "Upload TC09: Verify file upload can be cancelled",
      "Upload TC10: Verify multiple files can be selected when allowed",
      "Upload TC11: Verify file input accepts only allowed extensions",
      "Upload TC12: Verify file upload is responsive on mobile viewport",
      "Upload TC13: Verify file upload is accessible via keyboard",
      "Upload TC14: Verify file upload component has accessible label",
      "Upload TC15: Verify file upload page loads without errors",
      "Download TC01: Verify download starts on clicking the download button",
      "Download TC02: Verify downloaded file name matches expected value",
      "Download TC03: Verify downloaded file is not empty",
      "Download TC04: Verify downloaded file content matches expected data",
      "Download TC05: Verify download button has accessible label",
      "Download TC06: Verify download button is keyboard accessible",
      "Download TC07: Verify download link href attribute is correct",
      "Download TC08: Verify download works on different browsers",
      "Download TC09: Verify download is responsive on mobile viewport",
      "Download TC10: Verify multiple downloads can occur sequentially",
      "Download TC11: Verify download does not navigate away from page",
      "Download TC12: Verify download section is accessible via keyboard",
      "Download TC13: Verify file download page loads without errors",
      "Download TC14: Verify download file type matches expected MIME type",
    ],
  },
];

const detailedCases: DetailedCase[] = [
  {
    pageSlug: "forms",
    tc: "TC01",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Forms test case:

TC01: Fill all fields with valid data and submit successfully

1. Navigate to https://qaplayground.com/practice/forms.
2. Fill input-first-name with 'John'.
3. Fill input-last-name with 'Doe'.
4. Fill input-email with 'john@example.com'.
5. Fill input-phone with '9876543210'.
6. Fill input-dob with a valid date.
7. Select radio-gender-male.
8. Select 'India' from select-country dropdown.
9. Fill input-city with 'Mumbai'.
10. Fill input-password with 'pass123'.
11. Fill input-confirm-password with 'pass123'.
12. Check checkbox-terms.
13. Click submit-form-btn.
14. Assert the success state is visible by checking visible success text such as "Form submitted successfully" or submitted-name containing "John".

Do not record an assert where the first arg is a CSS selector like [data-testid="form-success-msg"]. The current recorder assert shape accepts role/name pairs or URL assertions only.`,
    extraConstraints: [
      "Prefer the named data-testid values from the steps when accessible names are ambiguous.",
      "Fixed form values are literals. Do not use fill-unique.",
      "For the final success assertion, use a role/name or visible text assertion that replay can resolve; do not encode a CSS selector as an assert role.",
    ],
  },
];

function tcNumber(title: string): string {
  const match = title.match(/TC(?:-[A-Z]+)?-?(\d+)/i);
  return match?.[1] || "00";
}

function tcPrefix(title: string): string {
  const named = title.match(/^TC-([A-Z]+)-\d+/i)?.[1]?.toLowerCase();
  if (named) return `${named}-`;
  if (/^Upload\s+TC/i.test(title)) return "upload-";
  if (/^Download\s+TC/i.test(title)) return "download-";
  return "";
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/&/g, "and")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 80);
}

function pageUrl(page: PracticePage): string {
  return `https://qaplayground.com${page.path}`;
}

function detailedCaseFor(page: PracticePage, title: string): DetailedCase | undefined {
  const tc = `TC${tcNumber(title).padStart(2, "0")}`;
  return detailedCases.find((item) => item.pageSlug === page.slug && item.tc === tc);
}

function qaplaygroundCase(page: PracticePage, title: string): EvalCase {
  const url = pageUrl(page);
  const tc = `${tcPrefix(title)}tc${tcNumber(title).padStart(2, "0")}`;
  const detail = detailedCaseFor(page, title);
  const cleanTitle = title.replace(/^((Upload|Download)\s+)?TC(?:-[A-Z]+)?-?\d+:\s*/i, "");
  return {
    id: `qaplayground-${page.slug}-${tc}-${slugify(cleanTitle)}`,
    suite: "qaplayground",
    page: page.slug,
    name: `QA Playground ${page.label} ${title}`,
    prompt: detail?.prompt || `Use the agent-qa skill to record and replay this QA Playground test case:

${title}

1. Navigate to ${url}.
2. Perform the user interactions needed for this test case.
3. Assert the expected result for this test case.
4. Replay the scenario successfully.`,
    extraConstraints: [
      "This eval case should cover exactly one documented QA Playground test case, not the entire page.",
      "Prefer stable data-testid-backed selectors when the test case names them or when accessible names are ambiguous.",
      ...(detail?.extraConstraints || []),
    ],
    scenarioMatches(scenario: unknown): boolean {
      const text = textOf(scenario);
      return text.includes(url.toLowerCase()) &&
        cleanTitle
          .toLowerCase()
          .split(/\s+/)
          .filter((word) => word.length >= 4)
          .slice(0, 4)
          .some((word) => text.includes(word));
    },
  };
}

function qaplaygroundPageCase(page: PracticePage): EvalCase {
  const url = pageUrl(page);
  return {
    id: `qaplayground-${page.slug}-page-load`,
    suite: "qaplayground",
    page: page.slug,
    name: `QA Playground ${page.label} page loads`,
    prompt: `Use the agent-qa skill to record and replay this QA Playground page-load smoke test:

1. Navigate to ${url}.
2. Verify the page loads without errors.
3. Verify the page content includes ${page.smokeTerms.map((item) => `"${item}"`).join(" and ")}.
4. Replay the scenario successfully.`,
    extraConstraints: [
      "Keep this as a page-smoke scenario. Do not interact with every widget on the page.",
      "Use a URL or present-content assertion that proves the intended practice page loaded.",
    ],
    scenarioMatches(scenario: unknown): boolean {
      const text = textOf(scenario);
      return text.includes(url.toLowerCase()) &&
        page.smokeTerms.some((term) => text.includes(term));
    },
  };
}

const qaplaygroundCases = practicePages.flatMap((page) => [
  qaplaygroundPageCase(page),
  ...page.cases.map((title) => qaplaygroundCase(page, title)),
]);

export const cases: EvalCase[] = [
  {
    id: "saucedemo-checkout",
    suite: "saucedemo",
    name: "Saucedemo checkout reaches final confirmation",
    prompt: `Use the agent-qa skill to record and replay this full checkout flow:

1. Navigate to https://www.saucedemo.com/
2. Log in with username standard_user and password secret_sauce.
3. Add exactly one item to the cart.
4. Open the cart.
5. Start checkout.
6. Fill checkout information with first name Test, last name User, and postal code 12345.
7. Continue checkout.
8. Finish checkout.
9. Verify the final checkout confirmation page is reached.`,
    extraConstraints: [
      "This scenario starts at a public login page. Do not bootstrap profiles.",
      "Fixed values are not unique. Do not use fill-unique for standard_user, secret_sauce, Test, User, or 12345.",
    ],
    scenarioMatches(scenario: unknown): boolean {
      const text = textOf(scenario);
      return text.includes("saucedemo.com") &&
        text.includes("standard_user") &&
        text.includes("secret_sauce") &&
        text.includes("add to cart") &&
        text.includes("checkout") &&
        text.includes("checkout-complete");
    },
  },
  ...qaplaygroundCases,
];

export function selectCases(caseId?: string, suite?: string, page?: string): EvalCase[] {
  let selected = cases;
  if (caseId) selected = selected.filter((item) => item.id === caseId);
  if (suite) selected = selected.filter((item) => item.suite === suite);
  if (page) selected = selected.filter((item) => item.page === page);
  return selected;
}
