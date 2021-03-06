console.log(DATA);

function format_time(secs) {
    return `${secs.toFixed(4)}s`;
}

function format_size(bytes) {
    if (bytes >= 1073741824) {
        return (bytes / 1073741824).toFixed(2) + " GiB";
    }
    else if (bytes >= 1048576) {
        return (bytes / 1048576).toFixed(2) + " MiB";
    }
    else if (bytes >= 1024) {
        return (bytes / 1024).toFixed(2) + " KiB";
    }
    else if (bytes > 1) {
        return bytes + " bytes";
    }
    else if (bytes == 1) {
        return bytes + " byte";
    }
    else if (bytes == 0) {
        return "0 bytes";
    }
    throw "unable to convert bytes";
}

function change(average, first, b) {
    if (!b) {
        return "";
    }
    let change = (average / first - 1) * 100;
    if (average == first) {
        change = 0;
    }
    let klass = change_class(change);
    if (change == Infinity) {
        change = "---------";
    } else {
        change = change.toFixed(2) + '%';
    }
    return `<td class=${klass}>${change}</td>`;
}

function change_class(c) {
    if (c > 1) {
        return 'positive';
    } else if (c > 0.1) {
        return 'slightly-positive';
    } else if (c < -1) {
        return 'negative';
    } else if (c < -0.1) {
        return 'slightly-negative';
    } else {
        return '';
    }
}

function average_by(array, f = (a) => a) {
    return array.reduce((accumulator, current) => accumulator + f(current), 0) / array.length;
}

function max_rss(time) {
    return time.reduce((accumulator, current) => {
        let rss = Math.max(parseFloat(current.after_rss), parseFloat(current.before_rss));
        return Math.max(accumulator, rss);
    }, 0);
}

let benchs = "";

for (const bench of DATA.benchs) {
    benchs += `<h3 id="${bench.name}">Details of <b>${bench.name}</b></h3><table><tr><th rowspan="2">Stage</th>`

    for (let i = 0; i < bench.builds.length; i++) {
        const config = bench.builds[i];
        benchs += `<th colspan="${i > 0 ? 2 : 1}">${config.build}</th>`
    }

    for (let i = 0; i < bench.builds.length; i++) {
        const config = bench.builds[i];
        benchs += `<th colspan="${i > 0 ? 2 : 1}">${config.build}</th>`
    }

    benchs += "</tr><tr>";

    for (let i = 0; i < bench.builds.length; i++) {
        benchs += `<th class="r">Time</th>`;
        if (i > 0) {
            benchs += `<th class="r">%</th>`;
        }
    }

    for (let i = 0; i < bench.builds.length; i++) {
        benchs += `<th class="r">Memory</th>`;
        if (i > 0) {
            benchs += `<th class="r">%</th>`;
        }
    }

    benchs += "</tr>";

    let data = bench.builds.map(build => {
        let entries = {};

        for (const instance of build.times) {
            for (const data of instance) {
                if (entries[data.name] === undefined) {
                    entries[data.name] = [];
                }
                entries[data.name].push(data);
            }
        }

        let entries_avg = {};

        for (const entry in entries) {
            entries_avg[entry] = {
                time: average_by(entries[entry], entry => entry.time),
                rss: average_by(entries[entry], entry => parseFloat(entry.after_rss)),
            };
        }

        return entries_avg;
    });

    let times = bench.builds[0].times[0].map(entry => entry.name).filter(entry => {
        return data.every(build => {
            return build[entry] != undefined;
        });
    });

    for (let i = 0; i < times.length; i++) {
        const config = times[i];
        benchs += `<tr><th class="event">${config}</th>`;

        let first = data[0][config].time;

        for (let j = 0; j < data.length; j++) {
            const build = data[j];
            let average = build[config].time;
            benchs += `<td>${average.toFixed(4)}s</td>`;
            benchs += change(average, first, j > 0);
        }

        let first_rss = data[0][config].rss;

        for (let j = 0; j < data.length; j++) {
            const build = data[j];
            let average = build[config].rss;
            benchs += `<td>${average.toFixed(2)} MiB</td>`;
            benchs += change(average, first_rss, j > 0);
        }

        benchs += `</tr>`
    }

    benchs += `</table>`;
}

function escapeHTML(str) {
    var p = document.createElement("p");
    p.innerText = str
    return p.innerHTML;
}

function build_details() {
    const BENIGN_OPTS = ['changelog-seen', 'rust.deny-warnings', 'rust.deny-warnings', 'build.low-priority'];

    function linearize(keys, path, out) {
        for (const key in keys) {
            let new_path = path.concat([key]);
            if (typeof keys[key] === 'object') {
                linearize(keys[key], new_path, out)
            } else {
                if (!BENIGN_OPTS.includes(new_path.join('.'))) {
                    out.push([new_path, keys[key]]);
                }
            }
        }
    }

    for (const build of DATA.builds) {
        let out = [];
        linearize(build.config, [], out);
        build.config_linearized = out;
    }

    let common_opts = DATA.builds[0].config_linearized.filter(opt => {
        return DATA.builds.every(build => {
            return build.config_linearized.find(build_opt => JSON.stringify(opt) == JSON.stringify(build_opt)) !== undefined
        });
    });

    if (DATA.builds.length == 1) {
        common_opts = []
    }

    let result = ``;

    if (common_opts.length > 0) {
        result += `<div class="build-container"><div class="build"><h3>Common build options</h3>`;
        for (const opt of common_opts) {
            result += `<div class="split"><p>${opt[0].join(".")}:</p><p><b>${escapeHTML(JSON.stringify(opt[1]))}</b></p></div>`;
        }
        result += `</div>`;
    }

    for (let i = 0; i < DATA.builds.length; i++) {
        let build = DATA.builds[i];
        result += `<div class="build"><h3>Build <b>${build.name}</b></h3>`;
        result += `<div class="split"><p>Git commit:</p><p><b>${build.commit_short}</b></p></div>`;
        result += `<div class="split"><p>Git branch:</p><p><b>${build.branch}</b></p></div>`;
        result += `<div class="split"><p>Triple:</p><p><b>${build.triple}</b></p></div>`;
        result += `<div class="split"><p>From repo:</p><p><b>${build.repo}</b> at ${build.repo_path}</p></div>`;

        let opts = build.config_linearized.filter(opt => common_opts.find(common_opt => JSON.stringify(opt) == JSON.stringify(common_opt)) === undefined);

        if (opts.length > 0) {

            result += `<div class="extra-opts"><h4>Additional build options:</h4>`;

            for (const opt of opts) {
                result += `<div class="split"><p>${opt[0].join(".")}:</p><p><b>${escapeHTML(JSON.stringify(opt[1]))}</b></p></div>`;

            }
            result += `</div>`;
        }

        result += `</div>`;
    }

    result += `</div>`;

    return result;
}

function diff_table(data) {
    let result = `<table><tr><th rowspan="2">${data.type}</th>`;

    for (const column of data.columns) {
        for (let i = 0; i < DATA.builds.length; i++) {
            const build = DATA.builds[i];
            result += `<th colspan="${i > 0 ? 2 : 1}" class="bh">${build.name}</th>`
        }
    }

    result += "</tr><tr>";

    for (const column of data.columns) {
        for (let i = 0; i < DATA.builds.length; i++) {
            result += `<th class="r">${column.name}</th>`;
            if (i > 0) {
                result += `<th class="r">%</th>`;
            }
        }
    }

    result += "</tr>";

    for (const row of data.rows) {
        result += `<tr><th>${row.name}</th>`;

        for (let i = 0; i < row.columns.length; i++) {
            const column = row.columns[i];
            let first = column[0];
            let format = data.columns[i].format;

            for (let j = 0; j < DATA.builds.length; j++) {
                result += `<td>${format(column[j])}</td>`
                if (j > 0) {
                    let change = (column[j] / first - 1) * 100;
                    result += `<td class=${change_class(change)}> ${change.toFixed(2)}%</td>`;
                }
            }
        }
        result += "</tr>";
    }
    result += "</table>";

    return result;
}

function summary() {
    let summary = {
        type: 'Benchmark',
        columns: [{ name: 'Time', format: format_time }, { name: 'Memory', format: format_size }],
        rows: DATA.benchs.map(bench => {
            let times = bench.builds.map(build => average_by(build.time));
            let rss = bench.builds.map(build => average_by(build.times, time => max_rss(time) * 1024 * 1024));
            return { name: `<a href="#${bench.name}">${bench.name}</a>`, columns: [times, rss] };
        })
    };

    return `<h3>Build comparison</h3>${diff_table(summary)}`;
}

let dbg_filter = file => !file.path.endsWith(".pdb");

let std_sizes = DATA.builds.map(build => {
    return build.files.filter(dbg_filter)
        .filter(file => file.path.replaceAll("\\", "/").startsWith(`lib/rustlib/${build.triple}/lib/`))
        .reduce((a, b) => a + b.size, 0);
});

let compiler_sizes = DATA.builds.map(build => {
    return build.files.filter(dbg_filter)
        .filter(file => file.path.replaceAll("\\", "/").startsWith("bin/"))
        .reduce((a, b) => a + b.size, 0);
});

let total_sizes = DATA.builds.map(build => {
    return build.files.filter(dbg_filter).reduce((a, b) => a + b.size, 0);
});

let total_with_dbg_sizes = DATA.builds.map(build => {
    return build.files.reduce((a, b) => a + b.size, 0);
});

let size = {
    type: 'Build size',
    columns: [{ name: 'Size', format: format_size }],
    rows: [
        { name: 'Compiler size', columns: [compiler_sizes] },
        { name: 'Std size', columns: [std_sizes] },
        { name: 'Total size', columns: [total_sizes] },
        { name: 'Total with debug info', columns: [total_with_dbg_sizes] }
    ]
};

const build_sizes = `<h3>Build comparison</h3>${diff_table(size)}`;

let files = {};

for (const build of DATA.builds) {
    for (const file of build.files) {
        files[file[0]] = true;
    }
}

console.log(Object.keys(files));

let file_size = {
    type: 'Build size',
    columns: [{ name: 'Size', format: format_size }],
    rows: [
        { name: 'Compiler size', columns: [compiler_sizes] },
        { name: 'Std size', columns: [std_sizes] },
        { name: 'Total size', columns: [total_sizes] },
        { name: 'Total with debug info', columns: [total_with_dbg_sizes] }
    ]
};

const file_sizes = `<h3>Build file size details</h3>${diff_table(file_size)}`;

let title = `Benchmark result for `;

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    if (i > 0) {
        title += ` vs. `;
    }
    title += `<b>${DATA.benchs[0].builds[i].build}</b>`;
}


document.body.innerHTML = `<div><h1>${title}</h1>${summary()}${build_details()}${build_sizes}${benchs}</div>`;
