console.log("Report JSON", DATA);

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

function bench_detail(bench) {
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
                rss: average_by(entries[entry], entry => parseFloat(entry.after_rss) * 1024 * 1024),
            };
        }

        return entries_avg;
    });

    let times = bench.builds[0].times[0].map(entry => entry.name).filter(entry => {
        return data.every(build => {
            return build[entry] != undefined;
        });
    });

    console.log(data);

    let table = {
        type: 'Stage',
        columns: [{ name: 'Time', format: format_time }, { name: 'Memory', format: format_size }],
        rows: times.map(event => {
            return {
                name: event,
                columns: [data.map(build => build[event].time), data.map(build => build[event].rss)]
            };
        })
    };

    return `<div><h3 id="${bench.name}">Details of <b>${bench.name}</b></h3>${diff_table(table)}</div>`;
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

    return `<div><h3>Benchmark summary</h3>${diff_table(summary)}</div>`;
}

const build_sizes = (() => {
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
        type: '',
        columns: [{ name: 'Size', format: format_size }],
        rows: [
            { name: 'Compiler size', columns: [compiler_sizes] },
            { name: 'Std size', columns: [std_sizes] },
            { name: 'Total size', columns: [total_sizes] },
            { name: 'Total with debug info', columns: [total_with_dbg_sizes] }
        ]
    };

    return `<div><h3>Build size comparison</h3>${diff_table(size)}</div>`;
})();

let file_map = {};

for (const build of DATA.builds) {
    for (const file of build.files) {
        file_map[file.path] = true;
    }
}

let files = Object.keys(file_map).sort();

let compiler_sizes = DATA.builds.map(build => {
    return build.files
        .filter(file => file.path.replaceAll("\\", "/").startsWith("bin/"))
        .reduce((a, b) => a + b.size, 0);
});

let file_size = {
    type: 'File',
    columns: [{ name: 'Size', format: format_size }],
    rows: files.map(file => {
        return {
            name: file, columns: [DATA.builds.map(build => {
                let f = build.files.find(f => f.path === file);
                return f == undefined ? 0 : f.size;
            })]
        };
    })
};

const file_sizes = `<div><h3>Build size details</h3>${diff_table(file_size)}</div>`;

let title = `Benchmark result for `;

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    if (i > 0) {
        title += ` vs. `;
    }
    title += `<b>${DATA.benchs[0].builds[i].build}</b>`;
}

let content = `<div>`;
content += `<h1>${title}</h1><p>Results are the average of ${DATA.benchs[0].builds[0].time.length} execution(s).</p>`;
content += `<div class="flex">`;
content += summary();
content += build_sizes;
content += `</div>`;
content += build_details();
content += `<div class="flex">`;
content += file_sizes;
for (const bench of DATA.benchs) {
    content += bench_detail(bench);
}
content += `</div>`;
content += `</div>`;
document.body.innerHTML = content;
