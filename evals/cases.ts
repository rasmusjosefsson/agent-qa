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
  prefix?: string;
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

const fileUploadUrl = "https://qaplayground.com/practice/file-upload";
const uploadValidFixture = "evals/fixtures/upload-valid.txt";
const uploadUnsupportedFixture = "evals/fixtures/upload-unsupported.exe";

function fileUploadDetailedCase(
  group: "Upload" | "Download",
  tc: string,
  title: string,
  steps: string[],
  extraConstraints: string[] = [],
): DetailedCase {
  const numberedSteps = steps.map((step, index) => `${index + 1}. ${step}`).join("\n");
  return {
    pageSlug: "file-upload",
    tc: `TC${tc}`,
    prefix: `${group.toLowerCase()}-`,
    prompt: `Use the agent-qa skill to record and replay this QA Playground File Upload test case:

${group} TC${tc}: ${title}

${numberedSteps}

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert static tutorial or test-case documentation text as a substitute for the live upload/download component state.`,
    extraConstraints: [
      "This eval case should cover exactly one documented File Upload test case, not every upload and download behavior on the page.",
      "Treat Selenium sendKeys() and Playwright setInputFiles() mentions in the source TC as intent only; do not use Selenium, Playwright, Puppeteer, or their APIs.",
      "For file selection, drive the page with agent-browser upload and record action method uploadBySelector, for example args [\"#file-upload\", \"evals/fixtures/upload-valid.txt\"]. Do not record file selection as fillBySelector.",
      "Prefer data-testid or id selectors for the file input, upload button, download button, status messages, and file list when present.",
      ...extraConstraints,
    ],
  };
}

const fileUploadDetailedCases: DetailedCase[] = [
  fileUploadDetailedCase("Upload", "01", "Verify a file can be selected for upload", [
    `Navigate to ${fileUploadUrl}.`,
    `Locate the file input using data-testid or id, then select ${uploadValidFixture}.`,
    "Verify in the live browser that the selected file name upload-valid.txt appears in the upload field, label, or adjacent selected-file display.",
    "Record the upload action with uploadBySelector and replay the scenario successfully.",
    "If a replay-side filename assertion after upload times out, report the post-upload DOM-check framework gap instead of converting the upload action to fillBySelector.",
  ], [
    `Use the repo-relative fixture ${uploadValidFixture}; do not hard-code an absolute machine-specific file path in the prompt or scenario intent.`,
    "The current portable replay contract is the do/upload step. Verify filename display live before flushing; do not require a replay-side DOM text check if it times out after upload.",
  ]),
  fileUploadDetailedCase("Upload", "02", "Verify selected file name is displayed after selection", [
    `Navigate to ${fileUploadUrl}.`,
    `Select ${uploadValidFixture} with the file input.`,
    "Verify live that the displayed file name exactly matches upload-valid.txt.",
    "Replay the upload scenario successfully, or report the post-upload filename assertion gap if replay-side DOM checks time out.",
  ], [
    "Do not accept a broad page text match unless it comes from the live selected-file display, because the page may contain instructional text.",
    "Do not replace uploadBySelector with fillBySelector to make the filename assertion pass; that records typing, not file upload.",
  ]),
  fileUploadDetailedCase("Upload", "06", "Verify error message for unsupported file type", [
    `Navigate to ${fileUploadUrl}.`,
    `Select ${uploadUnsupportedFixture}.`,
    "Attempt to upload if the upload button becomes available.",
    "Assert a visible unsupported-file-type error appears.",
    "Replay the scenario successfully, or report that the page does not enforce unsupported extensions.",
  ], [
    `Use the repo-relative fixture ${uploadUnsupportedFixture}. It is intentionally tiny and named with an .exe extension for validation only.`,
  ]),
  fileUploadDetailedCase("Upload", "07", "Verify error message for files exceeding size limit", [
    `Navigate to ${fileUploadUrl}.`,
    "Read the page text or file input constraints to determine the stated size limit.",
    "Create a temporary oversized fixture under the eval result directory, larger than the stated limit, without committing it to the repo.",
    "Select the oversized file and attempt to upload.",
    "Assert a visible file-size error appears.",
    "Replay the scenario successfully, or report that no size limit is discoverable/enforced.",
  ], [
    "Do not add a large binary fixture to the repository. Generate oversized data only under the isolated eval results directory.",
  ]),
  fileUploadDetailedCase("Upload", "08", "Verify uploaded file appears in the file list", [
    `Navigate to ${fileUploadUrl}.`,
    `Upload ${uploadValidFixture}.`,
    "After upload succeeds, assert upload-valid.txt appears in the uploaded-file list or history section.",
    "Replay the scenario successfully.",
  ]),
  fileUploadDetailedCase("Upload", "09", "Verify file upload can be cancelled", [
    `Navigate to ${fileUploadUrl}.`,
    `Select ${uploadValidFixture} and begin upload.`,
    "Click the cancel button if one is available during upload.",
    "Assert the upload stops and the field resets or the selected-file display is cleared.",
    "Replay the scenario successfully, or report that no cancel control exists.",
  ], [
    "Do not fake cancellation by refreshing the page unless that is the documented cancel behavior on the component.",
  ]),
  fileUploadDetailedCase("Upload", "10", "Verify multiple files can be selected when allowed", [
    `Navigate to ${fileUploadUrl}.`,
    "Locate a file input that has the multiple attribute.",
    `Select ${uploadValidFixture} and ${uploadUnsupportedFixture} together only if multiple selection is allowed.`,
    "Assert both file names are displayed.",
    "Replay the scenario successfully, or report that the file input is single-file only.",
  ], [
    "Do not force multi-file selection if the input lacks the multiple attribute; report the page capability instead.",
  ]),
  fileUploadDetailedCase("Upload", "11", "Verify file input accepts only allowed extensions", [
    `Navigate to ${fileUploadUrl}.`,
    "Read the accept attribute of the file input.",
    "Assert the accept value contains only the expected allowed file types documented by the page.",
    "Replay the scenario successfully, or report if accept-attribute assertions are not record/replay supported.",
  ], [
    "Verify the accept attribute live with agent-browser before flushing; record the strongest replayable assertion available.",
  ]),
  fileUploadDetailedCase("Upload", "12", "Verify file upload is responsive on mobile viewport", [
    `Navigate to ${fileUploadUrl} at a 375px-wide mobile viewport.`,
    "Assert the upload control is fully visible and usable without horizontal scroll.",
    `Select and upload ${uploadValidFixture}.`,
    "Assert the success state is visible in the mobile viewport.",
    "Replay the scenario successfully, or report the viewport/scroll assertion gap.",
  ], [
    "Use a viewport-setting command if the current agent-browser skill exposes one; otherwise report the missing viewport capability instead of silently running desktop-only.",
  ]),
  fileUploadDetailedCase("Upload", "13", "Verify file upload is accessible via keyboard", [
    `Navigate to ${fileUploadUrl}.`,
    "Tab to the file input or its associated label/control.",
    "Press Enter or Space to activate the upload control.",
    "Assert the file dialog opens if the framework can observe native file dialogs, otherwise report the native file-dialog observability gap.",
    "Replay the scenario successfully when supported.",
  ], [
    "Native OS file dialogs are a framework boundary. Do not replace this with direct file selection unless you clearly report the keyboard-dialog gap.",
  ]),
  fileUploadDetailedCase("Upload", "14", "Verify file upload component has accessible label", [
    `Navigate to ${fileUploadUrl}.`,
    "Assert the file input has an associated label, aria-label, aria-labelledby, or equivalent accessible name.",
    "Assert a screen reader can identify the upload control from the accessible name or label relationship.",
    "Replay the scenario successfully, or report if accessibility-tree assertions are unsupported.",
  ], [
    "Prefer an ARIA/accessibility snapshot if available; otherwise verify the DOM label relationship live and record the strongest replayable selector assertion.",
  ]),
  fileUploadDetailedCase("Upload", "15", "Verify file upload page loads without errors", [
    `Navigate to ${fileUploadUrl}.`,
    "Assert the navigation succeeds and the URL remains /practice/file-upload.",
    "Assert no JavaScript console errors are present if console inspection is supported.",
    "Assert the upload input element is visible in the DOM.",
    "Replay the scenario successfully.",
  ], [
    "If HTTP status or console-error assertions are not record/replay supported, report that limitation and still record URL plus upload-input visibility checks.",
  ]),
  fileUploadDetailedCase("Download", "01", "Verify download starts on clicking the download button", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Locate the download button using data-testid, id, or accessible name.",
    "Configure the eval download directory or set up a download listener if supported.",
    "Click the download button.",
    "Assert a file is saved in the expected download directory.",
    "Replay the scenario successfully, or report the download-capture support gap.",
  ], [
    "Use the isolated eval result directory for downloads when possible; do not rely on the user's default Downloads folder.",
  ]),
  fileUploadDetailedCase("Download", "02", "Verify downloaded file name matches expected value", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Click the download button.",
    "Assert the downloaded file name matches the expected filename from the button/link download attribute or page text.",
    "Replay the scenario successfully, or report the download filename observability gap.",
  ]),
  fileUploadDetailedCase("Download", "03", "Verify downloaded file is not empty", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Click the download button.",
    "Read the downloaded file from the isolated download directory.",
    "Assert the file size is greater than 0 bytes.",
    "Replay the scenario successfully, or report the download-file inspection gap.",
  ]),
  fileUploadDetailedCase("Download", "04", "Verify downloaded file content matches expected data", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Click the download button.",
    "Read the downloaded file contents.",
    "Assert the content matches the expected data, sample text, or documented file format from the page.",
    "Replay the scenario successfully, or report the deterministic-content gap.",
  ]),
  fileUploadDetailedCase("Download", "05", "Verify download button has accessible label", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Assert the download button has descriptive visible text, aria-label, or aria-labelledby.",
    "Assert a screen reader can identify the download action from the accessible name.",
    "Replay the scenario successfully, or report if accessibility-tree assertions are unsupported.",
  ]),
  fileUploadDetailedCase("Download", "06", "Verify download button is keyboard accessible", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Tab to the download button.",
    "Press Enter to trigger the download.",
    "Assert the download starts or the download file appears in the isolated directory.",
    "Replay the scenario successfully, or report the keyboard/download-capture gap.",
  ]),
  fileUploadDetailedCase("Download", "07", "Verify download link href attribute is correct", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Read the href and/or download attribute of the download anchor or button-backed link.",
    "Assert the URL or file path is valid and matches the expected downloadable asset for this page.",
    "Replay the scenario successfully, or report if attribute assertions are unsupported.",
  ]),
  fileUploadDetailedCase("Download", "08", "Verify download works on different browsers", [
    `Navigate to ${fileUploadUrl} in each supported browser runner available to the eval environment.`,
    "Trigger the same download in Chrome, Firefox, and Edge if those browsers are supported by agent-qa/agent-browser in this environment.",
    "Assert the file downloads successfully on each browser.",
    "Assert the downloaded file content is identical across browsers.",
    "Replay the scenario successfully, or report the cross-browser support gap.",
  ], [
    "Do not claim cross-browser coverage from a Chrome-only run. If only Chrome/CDP is supported, report that this is a framework capability gap.",
  ]),
  fileUploadDetailedCase("Download", "09", "Verify download is responsive on mobile viewport", [
    `Navigate to ${fileUploadUrl} at a 375px-wide mobile viewport and locate the download section.`,
    "Assert the download button is visible and tappable without horizontal scroll.",
    "Trigger the download and assert it initiates.",
    "Replay the scenario successfully, or report the viewport/download support gap.",
  ]),
  fileUploadDetailedCase("Download", "10", "Verify multiple downloads can occur sequentially", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Click the download button twice in sequence.",
    "Assert two separate files are saved or that the same target file is deterministically overwritten according to browser behavior.",
    "Replay the scenario successfully, or report the download-capture support gap.",
  ]),
  fileUploadDetailedCase("Download", "11", "Verify download does not navigate away from page", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Record the current page URL.",
    "Click the download button.",
    "Assert the page URL is still /practice/file-upload after the download triggers.",
    "Replay the scenario successfully.",
  ]),
  fileUploadDetailedCase("Download", "12", "Verify download section is accessible via keyboard", [
    `Navigate to ${fileUploadUrl}.`,
    "Tab through the page until the download section is reached.",
    "Assert the download button receives visible focus.",
    "Replay the scenario successfully, or report if focus visibility assertions are unsupported.",
  ]),
  fileUploadDetailedCase("Download", "13", "Verify file download page loads without errors", [
    `Navigate to ${fileUploadUrl}.`,
    "Assert the navigation succeeds and the URL remains /practice/file-upload.",
    "Assert no JavaScript console errors are present if console inspection is supported.",
    "Assert the download button is visible.",
    "Replay the scenario successfully.",
  ], [
    "If HTTP status or console-error assertions are not record/replay supported, report that limitation and still record URL plus download-button visibility checks.",
  ]),
  fileUploadDetailedCase("Download", "14", "Verify download file type matches expected MIME type", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Intercept or inspect the download network request if supported.",
    "Assert the Content-Type header matches the expected file type for the downloadable asset.",
    "Replay the scenario successfully, or report the network-header/download interception support gap.",
  ], [
    "Do not infer MIME type only from the filename extension if response-header inspection is available.",
  ]),
];

const detailedCases: DetailedCase[] = [
  ...fileUploadDetailedCases,
  {
    pageSlug: "dropdowns",
    tc: "TC01",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC01: Select 'Apple' from fruit dropdown by visible text

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the fruit dropdown using id dropdown-fruit or data-testid dropdown-fruit.
3. Select Apple from the fruit dropdown.
4. Verify in the live browser that Apple is the selected fruit before flushing.
5. Record the strongest replayable assertion available, such as the fruit result display containing Apple or the selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-fruit\", \"apple\"].",
      "If the recorder cannot represent a native selected-option assertion directly, verify it live and record a replayable result/display assertion instead.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC02",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC02: Select 'India' from country dropdown by value attribute

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the country dropdown using id dropdown-country or data-testid dropdown-country.
3. Select the option whose value is india.
4. Verify in the live browser that the selected country text is India before flushing.
5. Record the strongest replayable assertion available, such as a country result display containing India or the selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-country\", \"india\"].",
      "The selected option value is india and the expected visible text is India.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC03",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC03: Verify selected value is displayed after selection

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Select Banana from the fruit dropdown using id dropdown-fruit or data-testid dropdown-fruit.
3. Locate the fruit result display using data-testid result-fruit.
4. Assert the result display text contains Banana.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert static page documentation text as a substitute for the result display.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-fruit\", \"banana\"].",
      "Prefer [data-testid=\"result-fruit\"] for the result check.",
      "Avoid a broad text assertion for Banana unless it is scoped or tied to the result display; the page may include option/tutorial text.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC04",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC04: Get all available options from the programming language dropdown

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the programming language dropdown using data-testid dropdown-language.
3. Verify in the live browser that the dropdown has exactly three options.
4. Verify the option texts include Python, Java, and JavaScript.
5. Record replayable checks that prove the language dropdown loaded and remains available.
6. Replay the scenario successfully, or report the exact recorder gap if option-count/text assertions cannot be represented.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as proof of the options.`,
    extraConstraints: [
      "Use selector #dropdown-language or [data-testid=\"dropdown-language\"] for live option checks.",
      "If count assertions are unsupported in record/replay, report that limitation and keep the scenario to the strongest replayable dropdown-presence checks.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC05",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC05: Select the last option from the programming language dropdown

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the programming language dropdown using data-testid dropdown-language.
3. Select the last option, JavaScript.
4. Verify in the live browser that JavaScript is selected before flushing.
5. Record the strongest replayable assertion available, such as a language result display containing JavaScript or the selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-language\", \"javascript\"].",
      "The expected final selection is JavaScript.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC06",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC06: Multi-select: select multiple superheroes

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the heroes multi-select using data-testid dropdown-heroes.
3. Select Ant-Man and Batman.
4. Verify in the live browser that exactly those two heroes are selected before flushing.
5. Record the strongest replayable assertion available, such as a heroes result display containing Ant-Man and Batman or selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the multi-select with agent-browser select, then record action method selectBySelector with args [\"#dropdown-heroes\", \"ant-man\"] and another selectBySelector action for \"batman\".",
      "The selected option values are ant-man and batman; the expected visible texts are Ant-Man and Batman.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC07",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC07: Multi-select: deselect a previously selected option

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Select Ant-Man and Aquaman from the heroes multi-select using data-testid dropdown-heroes.
3. Deselect Ant-Man so only Aquaman remains selected.
4. Verify in the live browser that Aquaman is selected and Ant-Man is not selected before flushing.
5. Record the strongest replayable assertion available, such as a heroes result display containing only Aquaman or selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the multi-select with agent-browser select. Record selectBySelector actions for [\"#dropdown-heroes\", \"ant-man\"], [\"#dropdown-heroes\", \"aquaman\"], then [\"#dropdown-heroes\", \"aquaman\"] for the final selected state if deselect gestures are not replayable.",
      "If native deselect gestures are not replayable, prefer setting the multi-select to only aquaman and report the limitation clearly.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC08",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC08: Verify default placeholder text before any selection

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the fruit dropdown before interacting with it using data-testid dropdown-fruit.
3. Verify the default placeholder or selected option text reads Select Fruit.
4. Record a replayable assertion that proves the initial fruit dropdown state is present.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not select any fruit for this test.`,
    extraConstraints: [
      "Use #dropdown-fruit or [data-testid=\"dropdown-fruit\"] for the initial state check.",
      "Avoid broad page-text assertions for Select Fruit unless scoped to the dropdown; docs or examples can duplicate that text.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC09",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC09: Verify a dropdown is enabled and interactable

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the country dropdown using data-testid dropdown-country.
3. Verify in the live browser that the dropdown is enabled.
4. Select a country option and verify the selection succeeds without error.
5. Record the strongest replayable assertion available for the successful country selection.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-country\", \"india\"] or another valid country value.",
      "If enabled-state assertions are unsupported in record/replay, verify enabled state live and record the successful selection path as replay evidence.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC10",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC10: Verify the total count of options in the country dropdown

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the country dropdown using data-testid dropdown-country.
3. Verify in the live browser that the country dropdown has exactly four options.
4. Verify the option texts are India, USA, UK, and Argentina.
5. Record replayable checks that prove the country dropdown loaded and remains available.
6. Replay the scenario successfully, or report the exact recorder gap if option-count/text assertions cannot be represented.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as proof of the options.`,
    extraConstraints: [
      "Use selector #dropdown-country or [data-testid=\"dropdown-country\"] for live option checks.",
      "If count assertions are unsupported in record/replay, report that limitation and keep the scenario to the strongest replayable dropdown-presence checks.",
    ],
  },
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
  {
    pageSlug: "alerts-dialogs",
    tc: "TC01",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC01: Accept a simple browser alert and verify it closes

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Simple Alert button using data-testid btn-simple-alert.
3. Accept the native browser alert.
4. Assert the alert is dismissed and the page remains interactive.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay alert acceptance.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not fake success by only clicking tutorial text or by asserting static page documentation.`,
    extraConstraints: [
      "Native browser alert acceptance is a framework boundary. If replay cannot represent it, report that gap instead of replacing the test with a DOM-only assertion.",
      "The page contains Selenium/Playwright tutorial text and test-case text; do not use that static documentation as proof that the alert was handled.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC02",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC02: Get text from a simple browser alert before accepting

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Simple Alert button using data-testid btn-simple-alert.
3. Read the native alert message and verify it is Welcome to QA PlayGround!.
4. Accept the alert after reading the text.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay alert text reads.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert static documentation text as a substitute for the browser alert message.`,
    extraConstraints: [
      "This case requires observing native alert text. If agent-qa lacks a read/assert dialog step, report the framework gap clearly.",
      "Do not record a broad text assertion for Welcome to QA PlayGround! unless it comes from the actual dialog handling path.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC03",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC03: Accept a confirm dialog and verify accepted state

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Confirm Alert button using data-testid btn-confirm-alert.
3. Accept the native confirm dialog.
4. Assert the result display shows Accepted.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay confirm acceptance.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use static test-case documentation text as the result assertion.`,
    extraConstraints: [
      "Native confirm acceptance is a framework boundary. If unsupported, report the gap instead of faking the Accepted state.",
      "Prefer selector-scoped or live DOM checks for the post-confirm result if dialog handling succeeds.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC04",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC04: Dismiss a confirm dialog and verify dismissed state

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Confirm Alert button using data-testid btn-confirm-alert.
3. Dismiss the native confirm dialog.
4. Assert the result display shows Dismissed.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay confirm dismissal.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use static test-case documentation text as the result assertion.`,
    extraConstraints: [
      "Native confirm dismissal is a framework boundary. If unsupported, report the gap instead of faking the Dismissed state.",
      "Prefer selector-scoped or live DOM checks for the post-confirm result if dialog handling succeeds.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC05",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC05: Enter text in a prompt dialog and accept it

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Prompt Alert button using data-testid btn-prompt-alert.
3. Type John Doe into the native prompt dialog and accept it.
4. Assert the prompt result display shows Your name is - John Doe.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay prompt input.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not fake the result by directly editing the DOM.`,
    extraConstraints: [
      "Native prompt input is a framework boundary. If unsupported, report the gap instead of replacing the prompt with a normal DOM fill.",
      "John Doe is a fixed literal for this test. Do not use fill-unique.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC06",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC06: Dismiss a prompt dialog and verify no input is captured

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Prompt Alert button using data-testid btn-prompt-alert.
3. Dismiss the native prompt dialog without entering text.
4. Assert the prompt result display is empty or not visible.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay prompt dismissal.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use static page documentation as the assertion.`,
    extraConstraints: [
      "Native prompt dismissal is a framework boundary. If unsupported, report the gap instead of faking an empty result.",
      "Prefer selector absence or scoped DOM text checks for the prompt result if dialog handling succeeds.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC07",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC07: Verify toast notification appears after triggering

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Toast Alert button using data-testid btn-toast-alert.
3. Immediately verify in the live browser that [data-sonner-toast] text contains This is simple toast.
4. Record a replayable wait for selector [data-sonner-toast].
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert the tutorial text that mentions toast notifications.`,
    extraConstraints: [
      "The toast is transient. A separate replay selectorText step can miss it after selector presence succeeds; verify text live before flushing and keep replay assertion to selector presence unless the framework gains atomic selector+text waiting.",
      "Do not use a broad text wait for This is simple toast because page documentation can contain similar text.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC08",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC08: Close a modal/sweet alert using the Cancel button

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Sweet Alert button using data-testid btn-modal-alert.
3. Wait for the alertdialog containing Modern Alert to appear.
4. Click the You Are! cancel button using data-testid btn-modal-cancel.
5. Assert the alertdialog is no longer visible.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use driver.switchTo().alert(); this is a custom DOM modal, not a native browser alert.`,
    extraConstraints: [
      "Use selector [role=\"alertdialog\"] for modal presence and absence; the stable cancel button is [data-testid=\"btn-modal-cancel\"].",
      "Avoid broad text assertions for Modern Alert because the docs can mention modal/sweet alert behavior.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC09",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC09: Close an advanced dialog using the Close button

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Share button using data-testid btn-dialog-share.
3. Wait for the dialog to open and assert [data-testid=\"input-share-link\"] is visible.
4. Assert the input value contains qaplayground.com/practice/alerts-dialogs.
5. Click the Close button using data-testid btn-dialog-close.
6. Assert the dialog is dismissed.
7. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert static page documentation text as a substitute for the share dialog state.`,
    extraConstraints: [
      "The share dialog uses role=dialog, input [data-testid=\"input-share-link\"], and close button [data-testid=\"btn-dialog-close\"].",
      "Because recorder assertions do not support input value assertions directly, verify the input value with a live DOM check before flushing and record selector presence/absence waits for replay.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC10",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC10: Verify alerts page loads without errors

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Assert the page URL is /practice/alerts-dialogs.
3. Assert the page content includes Alerts & Dialogs Automation Practice.
4. Assert all six trigger buttons are visible: btn-simple-alert, btn-confirm-alert, btn-prompt-alert, btn-toast-alert, btn-modal-alert, and btn-dialog-share.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only.`,
    extraConstraints: [
      "Use selector waits for the six trigger buttons rather than broad text checks, because the page also contains documentation and test-case text.",
      "If checking console errors or HTTP status is unsupported by record/replay, report that limitation and still record the strongest replayable page-load checks.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC01",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC01: Wait for a delayed browser alert to appear and accept it

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Click the Trigger Delayed Alert button using data-testid btn-delayed-alert or visible text.
3. Record an agent-qa wait long enough for the delayed alert to fire.
4. Assert the page remains responsive after the delayed alert path, proving the scenario did not timeout.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "This page contains Selenium/Playwright tutorial text; do not use those instructions as the scenario body.",
      "Native dialog handling is a framework boundary. If replay cannot represent the alert accept flow, report the framework gap instead of faking success.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC02",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC02: Wait for a hidden element to become visible after a delay

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Click the Show Element button using data-testid btn-show-element.
3. Record a wait for selector [data-testid="delayed-element"].
4. Assert that the live delayed element text is Element is now visible! before flushing.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "Prefer selector waits over broad text waits because the page includes tutorial and test-case documentation text.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC03",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC03: Wait for a disabled button to become enabled

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Verify the target button [data-testid="btn-enable-after-delay"] is initially disabled with a selector/state check.
3. Click the Activate Button control by visible text.
4. Record a wait for selector [data-testid="btn-enable-after-delay"]:not([disabled]).
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "Live inspection showed raw selector click did not start the countdown, but visible-text click did. Prefer clickByText for Activate Button.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC04",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC04: Wait for loading text to change to a loaded state

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Click the Load Data button using data-testid btn-load-data or visible text.
3. Record a selectorText wait scoped to main content for Data Loaded!.
4. Assert the live page reaches Data Loaded! before flushing.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "Do not use a broad text wait for Data Loaded! unless it is scoped; the docs also mention that text.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC05",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC05: Wait for a spinner to disappear before asserting completion

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Click the Start Spinner button using data-testid btn-start-spinner or visible text.
3. Record a wait that proves the spinner is gone or the done state is visible.
4. Assert the live page reaches the completion state before flushing.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "Prefer selector-scoped or live DOM checks over broad text waits because this page includes docs text about spinners.",
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
  const prefix = tcPrefix(title);
  return detailedCases.find((item) =>
    item.pageSlug === page.slug &&
    item.tc === tc &&
    (item.prefix === undefined || item.prefix === prefix)
  );
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
