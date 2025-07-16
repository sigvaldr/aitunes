// main.cpp

#include <iostream>
#include <fstream>
#include <string>
#include <map>
#include <vector>
#include <tuple>
#include <cstdlib>

#include <curl/curl.h>
#include <ncurses.h>
#include <vlc/vlc.h>
#include <nlohmann/json.hpp>

using json = nlohmann::json;

// Simple struct to hold a track
struct Track {
    std::string id;
    std::string name;
    std::string album;
    std::string artist;
};

// libcurl write callback
static size_t write_cb(void* contents, size_t size, size_t nmemb, void* userp) {
    auto* s = static_cast<std::string*>(userp);
    size_t total = size * nmemb;
    s->append(static_cast<char*>(contents), total);
    return total;
}

// Perform a GET and return JSON
json http_get_json(const std::string& url,
                   const std::map<std::string,std::string>& headers) {
    CURL* curl = curl_easy_init();
    if (!curl) throw std::runtime_error("curl_easy_init failed");
    std::string response;
    struct curl_slist* hdrs = nullptr;
    for (auto& [k,v] : headers) {
        hdrs = curl_slist_append(hdrs, (k + ": " + v).c_str());
    }
    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, hdrs);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_cb);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response);
    // allow self‐signed certs
    curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0L);
    curl_easy_perform(curl);
    curl_slist_free_all(hdrs);
    curl_easy_cleanup(curl);
    return json::parse(response);
}

// Perform a POST with JSON payload, return JSON
json http_post_json(const std::string& url,
                    const json& payload,
                    const std::map<std::string,std::string>& headers) {
    CURL* curl = curl_easy_init();
    if (!curl) throw std::runtime_error("curl_easy_init failed");
    std::string response;
    std::string body = payload.dump();
    struct curl_slist* hdrs = nullptr;
    // ensure JSON header
    hdrs = curl_slist_append(hdrs, "Content-Type: application/json");
    for (auto& [k,v] : headers) {
        hdrs = curl_slist_append(hdrs, (k + ": " + v).c_str());
    }
    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, hdrs);
    curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body.c_str());
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_cb);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response);
    curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0L);
    curl_easy_perform(curl);
    curl_slist_free_all(hdrs);
    curl_easy_cleanup(curl);
    return json::parse(response);
}

// Load JSON config from file, or prompt and save if missing
json load_config(const std::string& path) {
    std::ifstream in(path);
    if (in) {
        json j;
        in >> j;
        return j;
    }

    // Prompt interactively
    std::string server_url, username, password;
    std::cout << "Jellyfin server URL: ";
    std::getline(std::cin, server_url);
    std::cout << "Username: ";
    std::getline(std::cin, username);
    std::cout << "Password: ";
    std::getline(std::cin, password);

    json j = {
        {"server_url", server_url},
        {"username",   username},
        {"password",   password}
    };
    std::ofstream out(path);
    out << j.dump(2) << std::endl;
    return j;
}

// Authenticate via username/password (no API key support)
// Returns (accessToken, userId, baseUrl)
std::tuple<std::string,std::string,std::string>
authenticate(const json& cfg) {
    std::string base = cfg.at("server_url").get<std::string>();
    // strip trailing slash
    if (!base.empty() && base.back() == '/') base.pop_back();
    std::vector<std::string> candidates = { base, base + "/jellyfin" };

    json payload = {
        {"Username", cfg.at("username").get<std::string>()},
        {"Pw",       cfg.at("password").get<std::string>()}
    };
    auto hdrs = std::map<std::string,std::string>{
        {"X-Emby-Authorization",
         R"(MediaBrowser Client="TUI", Device="cli", DeviceId="caitunes", Version="1.0")"}
    };

    for (auto& cand : candidates) {
        try {
            auto resp = http_post_json(cand + "/Users/AuthenticateByName",
                                      payload, hdrs);
            std::string token = resp.value("AccessToken", "");
            auto user = resp.value("User", json::object());
            std::string user_id = user.value("Id", "");
            if (!token.empty() && !user_id.empty()) {
                return {token, user_id, cand};
            }
        } catch (...) {
            // try next candidate
        }
    }
    std::cerr << "Authentication failed. Check URL, username, and password.\n";
    std::exit(1);
}

// Fetch all audio tracks for the user
std::vector<Track> fetch_tracks(const std::string& base,
                                const std::string& token,
                                const std::string& user_id) {
    std::vector<Track> out;
    int start = 0, limit = 10000;
    auto hdrs = std::map<std::string,std::string>{
        {"X-Emby-Token", token}
    };
    while (true) {
        auto resp = http_get_json(
            base + "/Users/" + user_id + "/Items?IncludeItemTypes=Audio"
                   "&Recursive=true&SortBy=Album,SortName&SortOrder=Ascending"
                   "&StartIndex=" + std::to_string(start) +
                   "&Limit="      + std::to_string(limit),
            hdrs
        );
        auto items = resp.value("Items", json::array());
        if (items.empty()) break;
        for (auto& it : items) {
            out.push_back({
                it.value("Id",""),
                it.value("Name","Unknown"),
                it.value("Album","Unknown"),
                !it.value("AlbumArtist","").empty()
                  ? it.value("AlbumArtist","")
                  : (!it.value("Artists", json::array()).empty()
                        ? it["Artists"][0].value("Name","Unknown")
                        : std::string("Unknown"))
            });
        }
        if ((int)items.size() < limit) break;
        start += limit;
    }
    return out;
}

// Very simple ncurses UI: list tracks, arrow to navigate, enter to play
void build_ui(const std::vector<Track>& tracks) {
    initscr();
    cbreak();
    noecho();
    keypad(stdscr, TRUE);

    mvprintw(0, 0, "caitunes: %d tracks. Press q to quit.\n", (int)tracks.size());
    int row = 2;
    for (auto& t : tracks) {
        mvprintw(row++, 2, "%s - %s", t.artist.c_str(), t.name.c_str());
        if (row >= LINES - 2) break;
    }
    refresh();
}

void main_loop(libvlc_instance_t* vlc,
               std::vector<Track>& queue,
               const std::vector<Track>& all_tracks,
               const std::string& base,
               const std::string& token) {
    int ch;
    size_t idx = 0;
    libvlc_media_player_t* player = nullptr;

    while ((ch = getch()) != 'q') {
        switch (ch) {
            case KEY_DOWN:
                if (idx + 1 < all_tracks.size()) idx++;
                break;
            case KEY_UP:
                if (idx > 0) idx--;
                break;
            case '\n': {
                if (player) libvlc_media_player_stop(player);
                auto& t = all_tracks[idx];
                std::string url = base
                    + "/Audio/" + t.id
                    + "/universal?AudioCodec=mp3&Container=mp3&api_key="
                    + token;
                libvlc_media_t* m = libvlc_media_new_location(vlc, url.c_str());
                player = libvlc_media_player_new_from_media(m);
                libvlc_media_release(m);
                libvlc_media_player_play(player);
                break;
            }
            default:
                break;
        }
        // redraw list with highlight
        for (int i = 0; i < (int)all_tracks.size() && i < LINES - 2; i++) {
            if ((size_t)i == idx) attron(A_REVERSE);
            mvprintw(2 + i, 2, "%s - %s",
                     all_tracks[i].artist.c_str(),
                     all_tracks[i].name.c_str());
            if ((size_t)i == idx) attroff(A_REVERSE);
        }
        refresh();
    }

    if (player) {
        libvlc_media_player_stop(player);
        libvlc_media_player_release(player);
    }
    endwin();
}

int main(int argc, char** argv) {
    curl_global_init(CURL_GLOBAL_DEFAULT);

    // Load or prompt for config
    json cfg = load_config(std::string(getenv("HOME")) + "/.caitunes_config.json");

    // Authenticate and fetch library
    auto [token, user_id, base] = authenticate(cfg);
    auto tracks = fetch_tracks(base, token, user_id);

    // Initialize UI and queue
    build_ui(tracks);
    std::vector<Track> queue = tracks;

    // Init libVLC
    libvlc_instance_t* vlc = libvlc_new(0, nullptr);
    if (!vlc) {
        std::cerr << "Error initializing libVLC\n";
        return 1;
    }

    // Start main loop
    main_loop(vlc, queue, tracks, base, token);

    libvlc_release(vlc);
    curl_global_cleanup();
    return 0;
}

