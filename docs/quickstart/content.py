"""The prose every quick start shares, and the part each one holds alone.

One home for the shared sections, because five guides that repeat a paragraph diverge on
the first correction. A guide is a section list; the builder turns it into a page.
"""

# The interface is the same behind every route, so the worked example and the screenshots
# it names are written once and read by all of them.
WORKED_TRIAL = "subject01_trial1.force.txt"


from pathlib import Path

IMAGES = Path(__file__).resolve().parent / "img"
COLUMN_MILLIMETRES = 165.0
TALLEST_MILLIMETRES = 88.0


def figure(name, caption):
    """One screenshot, at its own size rather than stretched to the column.

    A capture is taken at two device pixels per CSS pixel, so its natural size on the page is
    half its pixel width. Printing a narrow rail at full column width magnifies it past what
    it was drawn for and makes it the loudest thing on the page, and a tall one pushes past
    the bottom of the page and leaves most of the previous one empty.

    Imported here rather than beside the other imports, so that reading the prose out of this
    module costs nothing but this module: the command checker imports it to hold the guides to
    the program, and asking it for an image library to do that would put the gate behind an
    install on every machine that runs it.
    """
    from PIL import Image

    pixels = Image.open(IMAGES / f"{name}.png").size
    millimetres = [side / 2 * 25.4 / 96 for side in pixels]
    width = min(millimetres[0], COLUMN_MILLIMETRES)
    if millimetres[1] * (width / millimetres[0]) > TALLEST_MILLIMETRES:
        width = millimetres[0] * TALLEST_MILLIMETRES / millimetres[1]
    return (
        f'<figure style="max-width:{width:.1f}mm"><img src="img/{name}.png" alt="">'
        f"<figcaption>{caption}</figcaption></figure>"
    )


def note(body, label="Worth knowing", watch=False):
    kind = " note--watch" if watch else ""
    return (
        f'<div class="note{kind}"><span class="note__label">{label}</span>{body}</div>'
    )


def step(number, title, body):
    return (
        f'<section class="step"><div class="step__head">'
        f'<span class="step__number">Step {number}</span>'
        f'<h3 class="step__title">{title}</h3></div>{body}</section>'
    )


def what_it_is():
    return """
<h2>What it does</h2>

<p>plateforce reads a force-plate recording and computes the numbers a jump study reports:
jump height, time to takeoff, flight time, impulse, power, and the rest. Every number it
gives you carries the rule that produced it, and opening that rule shows the papers behind
it.</p>

<p>That second half is the reason to use it. Ten published ways of computing one jump height
disagree by a median of 3.5 cm on the same 244 real trials, and the training effect the
study behind those trials was built to detect was 2.0 cm. Two jump heights are only
comparable when they were computed the same way, so plateforce keeps the record that makes
the comparison possible.</p>
"""


def before_you_start():
    return """
<h2>Before you start</h2>

<p>You need a file, and three facts about it.</p>

<p><strong>The file</strong> is a text file of force values, one row per sample. Tab, comma
or semicolon separated, or one value per row. Names ending <code>.txt</code>,
<code>.csv</code>, <code>.tsv</code>, <code>.dat</code> and <code>.asc</code> all open.</p>

<p><strong>The three facts</strong> are about the file rather than about jumping, and
plateforce asks for them once:</p>

<ol>
<li><strong>Which column holds the vertical force.</strong> plateforce draws every column in
your file, so you can pick the one shaped like a jump.</li>
<li><strong>How many samples per second the plate recorded.</strong> If your file has a time
column, plateforce works the rate out from it and says so. If it has not, ask whoever ran the
collection, or look in the software that wrote the file, and do not guess: reading a 1200 Hz
recording as 1000 Hz puts every velocity and impulse out by a fifth, and every height and
displacement, which go with the square of the rate, out by nearly half.</li>
<li><strong>How the file writes a sample it does not have.</strong> Some export software
writes <code>0</code> or <code>-1</code> where no measurement was taken. Three such rows in
one 244-trial study moved a published correlation by 0.16. If your file does this, say
which value it uses.</li>
</ol>
""" + note(
        "<p>No file yet? <strong>Open a demo trial</strong> on the first screen loads a "
        "countermovement jump and takes you straight to the results, so you can look "
        "around before your own data arrives.</p>"
    )


def the_five_minutes(entry_step):
    """The path from a file to a defensible number, screen by screen.

    `entry_step` is the one step that differs by route, because opening a file in a tab and
    opening one in an application window are the same act described two ways.
    """
    return (
        """
<h2>The first five minutes</h2>
"""
        + entry_step
        + step(
            2,
            "Answer the three questions",
            """
<p>plateforce shows you every column it found and asks which one carries the vertical
force. Type the sample rate, say how missing values are written, then press
<strong>Analyse this trial</strong>.</p>

<p><strong>Analyse this trial</strong> stays greyed out until the rate is there. That is
deliberate: the rate cannot be guessed from the numbers, and a wrong one is not visible in
the result. If your file has a time column plateforce fills the box in for you and says where
the number came from.</p>
"""
            + figure(
                "columns",
                "The sketch beside each column is that column's own data, so a force channel "
                "is easy to recognise. This file holds one column of 6,000 samples.",
            ),
        )
        + step(
            3,
            "Read the results",
            """
<p>Your trace appears with the landmarks drawn on it, and the numbers beside it.</p>
"""
            + figure(
                "trace",
                "Standing still, the countermovement, takeoff, and the landing.",
            )
            + """
<p>Two numbers sit above the rest because they are the ones most papers lead with. Under
each one is the rule that produced it.</p>
"""
            + figure(
                "results-provisional",
                "Marked provisional, because nobody has chosen the rules yet.",
            )
            + """
<p><strong>Provisional</strong> means plateforce picked a rule so it could show you
something, and is telling you that you have not chosen. Step 4 is where you choose.</p>
""",
        )
        + step(
            4,
            "Choose the rules",
            """
<p><strong>Method decisions</strong>, on the right, lists every choice on the way to your
numbers. Two are open on a jump height: how the standing weight is measured, and where the
jump is judged to start. Published work disagrees about both, which is why plateforce will
not quietly decide for you.</p>
"""
            + figure(
                "decisions",
                "Each choice names its rules, and each rule names where it comes from.",
            )
            + """
<p>Open a choice and pick a rule, or press <strong>Use recommended rules</strong> to take
the recommended one for every open choice at once. Either way it is recorded as a choice you
made, and the numbers recompute.</p>
"""
            + figure("results-settled", "The same two numbers, now under rules you chose."),
        )
        + step(
            5,
            "See how much the choice mattered",
            """
<p><strong>How much does the method choice move this number?</strong> runs every defensible
alternative over your own trial and reports the range.</p>
"""
            + figure(
                "spread",
                "Every combination of the rules on its path, run over this trial.",
            )
            + """
<p>This is the figure to quote beside a result when you want a reader to know how much of it
is measurement and how much is method. It is computed from your own trial, so it is yours to
report rather than one borrowed from a paper.</p>
""",
        )
        + step(
            6,
            "Do the whole folder",
            """
<p>Choosing a folder instead of a file loads every trial in it under the rules you just
chose. plateforce names the file ending it is treating as a trial and counts what it read,
so a stray file in the folder cannot join your results without saying so.</p>
"""
            + figure("batch", "One row per trial, and the table scrolls sideways for the rest.")
            + """
<p>A trial that cannot be analysed is listed by name, with the rule and the value that
declined it. Nothing is dropped quietly.</p>

<p>A trial can be analysed and still have a quantity its rule would not give a number for,
and the <code>refusal_code</code> column is where that is said. In the trials above, five
recordings stop while the athlete is still in the air, so the landing never arrives and the
two quantities measured from it are left empty on those rows with the reason beside them. The
count at the top is of trials, and none of these was declined.</p>
""",
        )
    )


RELEASES = "https://github.com/DrAlexHarrison/plateforce/releases"

# One shared Markdown source, one section swapped per shell, because three separate documents
# would answer the same question three ways within a week.
TERMINAL_INSTALL = {
    "any": f"""## Get the program

One file, from [the releases page]({RELEASES}). No installer, no compiler, no package
manager. Take the one your machine runs:

| machine | file |
|---|---|
| macOS, Apple Silicon or Intel | `plateforce-universal-macos` |
| Windows | `plateforce-x86_64-windows.exe` |
| Linux, x86-64 | `plateforce-x86_64-linux-static` |
| Linux, arm64 | `plateforce-aarch64-linux-static` |

On macOS and Linux, fetching it at the terminal is the smoothest route, because a file
downloaded with `curl` carries no quarantine attribute and nothing asks you to approve it.
On macOS:

```
curl -LO {RELEASES}/latest/download/plateforce-universal-macos
chmod +x plateforce-universal-macos
./plateforce-universal-macos version
```

On Linux:

```
curl -LO {RELEASES}/latest/download/plateforce-x86_64-linux-static
chmod +x plateforce-x86_64-linux-static
./plateforce-x86_64-linux-static version
```

If you downloaded it in a browser instead, macOS refuses to run it until you clear that
attribute once:

```
xattr -d com.apple.quarantine plateforce-universal-macos
```

On Windows, run it from PowerShell in the folder you saved it to:

```powershell
.\\plateforce-x86_64-windows.exe version
```

Either way it answers with its version. Put the file somewhere on your `PATH` and the rest of
this guide can say `plateforce` instead of the whole name.
""",
    "macos": f"""## Get the program

Take `plateforce-universal-macos` from [the releases page]({RELEASES}). One file, no
installer, no compiler, no package manager, and it runs on both Apple Silicon and Intel.

Fetch it at the terminal rather than in a browser. A file downloaded with `curl` carries no
quarantine attribute, so nothing asks you to approve it:

```
curl -LO {RELEASES}/latest/download/plateforce-universal-macos
chmod +x plateforce-universal-macos
./plateforce-universal-macos version
```

If you already downloaded it in a browser, macOS refuses to run it until you clear that
attribute once:

```
xattr -d com.apple.quarantine plateforce-universal-macos
```

Put it on your `PATH` so the rest of this guide can say `plateforce`:

```
mkdir -p ~/.local/bin
mv plateforce-universal-macos ~/.local/bin/plateforce
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

Open a new terminal, and `plateforce version` answers.

**Completions and manual pages.** `plateforce completions zsh --out-dir ~/.zfunc` writes the
completion script, and adding `fpath=(~/.zfunc $fpath)` above `compinit` in `~/.zshrc` puts it
to work. `plateforce man` writes the manual pages; macOS does not read `~/.local/share/man`
by default, so the command prints the `man -M` form that works immediately.
""",
    "windows": f"""## Get the program

Take `plateforce-x86_64-windows.exe` from [the releases page]({RELEASES}). One file, no
installer, and it does not ask for an administrator.

In PowerShell, in the folder you saved it to:

```powershell
.\\plateforce-x86_64-windows.exe version
```

Windows may hold the file as blocked because it came from the internet. One command clears
that:

```powershell
Unblock-File .\\plateforce-x86_64-windows.exe
```

Put it somewhere on your `PATH` so the rest of this guide can say `plateforce`:

```powershell
New-Item -ItemType Directory -Force "$HOME\\bin"
Move-Item .\\plateforce-x86_64-windows.exe "$HOME\\bin\\plateforce.exe"
[Environment]::SetEnvironmentVariable(
  "Path", [Environment]::GetEnvironmentVariable("Path", "User") + ";$HOME\\bin", "User")
```

Open a new PowerShell window, and `plateforce version` answers.

**Completions.** `plateforce completions powershell` prints the script; append it to the file
`$PROFILE` names and it loads with every new window.

**A note on quoting.** PowerShell treats `--set weighing.duration=1.0` as one word, which is
what plateforce wants, but a value containing a comma or a space needs quotes:
`--set 'onset.k=5'`.
""",
    "linux": f"""## Get the program

Take `plateforce-x86_64-linux-static` from [the releases page]({RELEASES}), or
`plateforce-aarch64-linux-static` on arm64. It is statically linked, so it needs no glibc, no
runtime and no installation, and it runs on distributions the desktop packages cannot reach.

```
curl -LO {RELEASES}/latest/download/plateforce-x86_64-linux-static
chmod +x plateforce-x86_64-linux-static
./plateforce-x86_64-linux-static version
```

Put it on your `PATH` so the rest of this guide can say `plateforce`:

```
mkdir -p ~/.local/bin
mv plateforce-x86_64-linux-static ~/.local/bin/plateforce
```

Most distributions already have `~/.local/bin` on the path. If `plateforce version` does not
answer in a new terminal, add `export PATH="$HOME/.local/bin:$PATH"` to `~/.bashrc`.

**Completions and manual pages.** `plateforce completions bash --out-dir
~/.local/share/bash-completion/completions` writes the completion script where bash looks for
it. `plateforce man` writes the manual pages into `~/.local/share/man`, which is on the
default manpath, so `man plateforce` works in a new terminal.
""",
}


def install(platform):
    """The one section that differs by route, and the only place a file name appears."""
    return {
        "macos": """
<h2>Install it</h2>

<p>Download <code>plateforce_{version}_universal.dmg</code> from
<a href="{releases}">the releases page</a>. Open it, and drag plateforce to Applications.
One file runs on both Apple Silicon and Intel Macs, and needs macOS 10.13 or newer.</p>

<p>Your file is read on your own machine. plateforce makes no network requests, so it works
the same on a plane as it does in the laboratory.</p>
""",
        "windows": """
<h2>Install it</h2>

<p>Download <code>plateforce_{version}_x64-setup.exe</code> from
<a href="{releases}">the releases page</a> and run it. It installs into your own user
profile and does not ask for an administrator, so you do not need your IT department.</p>

"""
        + note(
            "<p>Windows shows a blue <strong>Windows protected your PC</strong> dialog the "
            "first time, because the installer carries no purchased certificate. Choose "
            "<strong>More info</strong>, then <strong>Run anyway</strong>.</p>",
            label="The first launch",
            watch=True,
        )
        + """
<p>Windows 10 version 1809 or newer. Your file is read on your own machine, and plateforce
makes no network requests.</p>
""",
        "linux": """
<h2>Install it</h2>

<p>All three downloads are on <a href="{releases}">the releases page</a>.</p>

<p><strong>AppImage is the one to take if you are not sure.</strong> It needs no root and no
package manager, which matters on a machine somebody else administers.</p>

<pre><code>chmod +x plateforce_{version}_amd64.AppImage
./plateforce_{version}_amd64.AppImage</code></pre>

<p><strong>Debian and Ubuntu:</strong>
<code>sudo apt install ./plateforce_{version}_amd64.deb</code></p>

<p><strong>Fedora and openSUSE:</strong>
<code>sudo dnf install ./plateforce-{version}-1.x86_64.rpm</code></p>

<p>All three need glibc 2.35 or newer, which is Ubuntu 22.04, Debian 12 and anything later.
Your file is read on your own machine, and plateforce makes no network requests.</p>
""",
    }[platform]


def elsewhere(here):
    """Three routes, one interface, so a reader who changes machine is not starting over."""
    routes = {
        "browser": "in a browser tab at dralexharrison.github.io/plateforce/app/",
        "desktop": "as an application on macOS, Windows and Linux",
        "terminal": "at a terminal, where <code>plateforce analyse</code> takes the same "
        "choices as flags and writes the same record",
    }
    others = [text for key, text in routes.items() if key != here]
    return f"""
<h2>Where else it runs</h2>

<p>The same analysis, the same registry and the same record are available
{others[0]}, and {others[1]}. A result computed on one carries the record that lets another
reproduce it.</p>
"""


def getting_numbers_out():
    """The section a thesis depends on, and the one the interface had no answer for until
    the export landed."""
    return """
<h2>Getting your numbers out</h2>

<p><strong>Download results (ZIP)</strong> writes a zip file to your downloads folder. Inside
it is one table per kind of thing, and <code>results.csv</code> is the one to open in a
spreadsheet.</p>

<table>
<thead><tr><th>File</th><th>What is in it</th></tr></thead>
<tbody>
<tr><td><code>results.csv</code></td><td>One row per trial, one column per quantity</td></tr>
<tr><td><code>provenance.csv</code></td><td>Which rule and which values produced each number</td></tr>
<tr><td><code>descriptions.csv</code></td><td>The account each number gives of itself</td></tr>
<tr><td><code>refusals.csv</code></td><td>Every quantity that declined, with the rule and the reason</td></tr>
<tr><td><code>warnings.csv</code></td><td>What the run wants you to look at</td></tr>
<tr><td><code>signals.csv</code></td><td>The landmarks found in each trial</td></tr>
<tr><td><code>exclusions.csv</code></td><td>Anything a rule removed, and under which rule</td></tr>
<tr><td><code>run.json</code></td><td>The request, the registry, and the record for the whole run</td></tr>
</tbody>
</table>

<p>The whole set travels together on purpose. A file of numbers with nothing beside it saying
how they were computed is the thing this tool exists to prevent, so the record comes with the
results rather than being somewhere else you have to remember to fetch.</p>

<p>One trial gives you the same set with one row in it, from the same control beside the
results. These are the same files the terminal writes, byte for byte, so a result computed
here and one computed there are the same result.</p>
"""


def methods_section():
    return (
        """
<h2>What to write in your methods section</h2>

<p><strong>Analysis record</strong>, at the bottom of the screen, identifies the software and
the rule set your numbers came from: the version of plateforce, the sample rate the analysis
ran at and where that number came from, the revision of the method registry, a digest that
identifies that registry exactly, the plate settings this analysis was given, and the gravity
it was bound to. The rules you chose and the values they used sit under each number, and
travel in the downloaded archive.</p>
"""
        + figure("record", "The record travels with the result.")
        + """
<p>A sentence built from it looks like this, with your own rules in place of these:</p>

<pre><code>Jump height was computed from take-off velocity by the impulse-momentum
method (jumpheight.takeoff.impulse_momentum), with system weight from the
lowest-variance one-second window (bwepoch.adaptive_lowest_variance) and
movement onset at five standard deviations of that window's noise
(onset.threshold.noise_relative, k = 5), in plateforce {version}
(registry {registry_revision}, {registry_digest}).</code></pre>

<p><strong>Copy as Markdown</strong>, above the results, puts the whole record on your
clipboard, so you can paste it into a document and read the rule names off it rather than
copying them by hand. Every value each rule used comes with it, marked cited, measured or
assumed.</p>

<p>The rate is in the record, with a note of whether you typed it or plateforce read it from
a time column. The force column you chose and the missing-value convention you declared are
yours to state, and they belong in the same sentence as the rules.</p>
"""
    )


def troubleshooting(extra=""):
    return (
        """
<h2>When something does not work</h2>

<dl>
<dt>It will not open my file</dt>
<dd>plateforce reads numbers, and it skips whatever sits above them: units, dates, a row of
column names. If it still cannot read the file it says what it found and where. The usual
cause is a file that is still an Excel workbook rather than a saved CSV or text file.</dd>

<dt>I do not know the sample rate</dt>
<dd>If your file has a time column, plateforce fills it in and tells you where the number came
from. If it has not, do not guess: it is in the collection software's settings, in the study
protocol, or with whoever ran the session. The consequence of a wrong one is not subtle, and
it is silent.</dd>

<dt>A trial declined</dt>
<dd>The message names the rule and the value that declined it, for instance a threshold that
was never crossed inside the search window. That usually means the trial is not the movement
the rule expects, or the recording was trimmed before the event. Try another rule for that
step and see whether it finds it.</dd>

<dt>My two jump heights disagree</dt>
<dd>Jump height from take-off velocity and jump height from flight time are two different
measurements, not two estimates of one. They answer different questions and they are
reported separately for that reason. The record says which is which, and the spread panel
says how far apart they are on your data.</dd>

<dt>The numbers changed when I changed a rule</dt>
<dd>That is the thing this tool exists to show you. The spread panel puts a size on it.</dd>
</dl>
"""
        + extra
    )


def glossary():
    return """
<h2>Words you will meet</h2>

<dl>
<dt>System weight</dt>
<dd>What the plate reads while the athlete stands still, including anything they are
holding. Not the same as bodyweight, which is why plateforce never calls it that.</dd>

<dt>Movement onset</dt>
<dd>The instant the jump starts. Published rules disagree about where that is, which is why
it is one of the two choices you are asked to make.</dd>

<dt>Takeoff</dt>
<dd>The instant the feet leave the plate.</dd>

<dt>Impulse</dt>
<dd>Force above standing weight, added up over time. Jump height from take-off velocity
comes from this.</dd>

<dt>RSI modified</dt>
<dd>Jump height for each unit of time spent on the ground getting there.</dd>

<dt>Provisional</dt>
<dd>A number computed under a rule you have not chosen yet.</dd>

<dt>Registry</dt>
<dd>The list of published rules plateforce carries, each with its citation, its status and
what it is known to do to a result.</dd>

<dt>Plate</dt>
<dd>Five facts about the plate itself, such as any filtering applied before the recording
was written. Answering them makes two sessions on the same equipment comparable. Left blank,
the record says they are unanswered rather than guessing.</dd>
</dl>
"""
